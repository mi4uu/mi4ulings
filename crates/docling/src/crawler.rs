//! Web crawler functionality for downloading web pages and extracting links
//!
//! This module provides functionality to:
//! - Crawl websites to a specified depth using the `spider` crate
//! - Download and save HTML content
//! - Extract and download media files (images)
//! - Support configurable parameters like crawl depth, delay, user agent, robots.txt respect
//! - Handle URL normalization and conversion to filenames
//!
//! The main entry point is the `Crawler` struct, which orchestrates the entire
//! crawling process for a given URL entry.

use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use spider::{
    configuration::{Configuration, RequestConfig}, // Import RequestConfig
    website::Website,
};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Semaphore, broadcast, mpsc};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::{CrawlStatus, DoclingConfig, UrlEntry};

/// Represents a web page with its URL and HTML content
///
/// This is used for communication between crawler components.
#[derive(Clone)]
struct Page {
    /// The URL of the page
    url: Url,
    /// The HTML content of the page
    body: String,
}

/// Crawler handles web crawling and content downloading using the `spider` crate
///
/// This is the main component that orchestrates the crawling process:
/// 1. Configures and initializes a spider `Website` for a URL entry
/// 2. Crawls the website to the specified depth, respecting robots.txt if configured
/// 3. Downloads and saves HTML content for allowed pages
/// 4. Extracts and downloads media files (images) from downloaded pages
/// 5. Updates the URL entry with status information
pub struct Crawler {
    /// Configuration for the crawler
    config: DoclingConfig,
    /// HTTP client for making requests (used for image downloads)
    client: Client,
}

impl Crawler {
    /// Creates a new crawler with the given configuration
    ///
    /// # Arguments
    /// * `config` - The configuration for the crawler
    ///
    /// # Returns
    /// A new Crawler instance if successful
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created
    pub fn new(config: DoclingConfig) -> Result<Self> {
        // Create HTTP client with user agent and other settings for image downloads
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(30)) // Timeout for image downloads
            .build()
            .context("Failed to create HTTP client for media")?;

        Ok(Self { config, client })
    }

    /// Processes a URL entry, downloading content and finding links
    ///
    /// This is the main entry point for crawling a website. It:
    /// 1. Updates the entry's status
    /// 2. Creates necessary output directories
    /// 3. Configures and starts the `spider` crate crawler
    /// 4. Downloads all pages and media files
    /// 5. Updates the entry with success/failure information
    ///
    /// # Arguments
    /// * `entry` - The URL entry to process
    ///
    /// # Returns
    /// Ok(()) if successful
    ///
    /// # Errors
    /// Returns an error if any step in the crawling process fails
    pub async fn process_entry(&mut self, entry: &mut UrlEntry) -> Result<()> {
        // Update entry status
        entry.last_try = Some(Utc::now());

        // Skip if disabled
        if entry.status == CrawlStatus::Disabled {
            info!("Skipping disabled entry: {}", entry.name);
            return Ok(());
        }

        info!("Processing entry: {} ({})", entry.name, entry.url);

        // Create output directories
        let base_output_dir = self.config.outputs_path.join(&entry.name);
        let html_output_dir = base_output_dir.join(&self.config.output_parts_html_suffix);
        let media_output_dir = base_output_dir.join(&self.config.output_parts_media_suffix);

        create_dir_all(&html_output_dir).context("Failed to create HTML output directory")?;
        create_dir_all(&media_output_dir).context("Failed to create media output directory")?;

        // Create error directory if it doesn't exist
        let error_dir = base_output_dir.join("ERRORS");
        create_dir_all(&error_dir).context("Failed to create error directory")?;

        // Configure the spider Website
        let request_config = RequestConfig::new()
            .with_user_agent(Some(&self.config.user_agent))
            .with_timeout(Some(Duration::from_secs(30)))
            .build();

        let spider_config = Configuration::new()
            .with_respect_robots_txt(self.config.respect_robots_txt)
            .with_delay(self.config.delay_between_request_in_ms)
            .with_request_config(Some(request_config))
            .with_max_depth(entry.crawl_depth as usize)
            .with_max_concurrent_requests(Some(self.config.max_concurrent_requests as usize))
            .with_subdomains(false) // Only crawl the specified domain
            .with_tld(false) // Only crawl the specified domain
            .build();

        let mut website = Website::new(&entry.url)
            .with_config(spider_config)
            .build()
            .context("Failed to build Website crawler")?;

        // Subscribe to receive pages
        let mut rx = website.subscribe(100)?; // Buffer size 100

        // Channel to send downloaded pages for image processing
        let (page_proc_tx, mut page_proc_rx) = mpsc::channel::<Page>(100);

        // Task to handle image downloading and saving HTML
        let html_dir = html_output_dir.clone();
        let media_dir = media_output_dir.clone();
        let client = self.client.clone();
        let config = self.config.clone();
        let download_task = tokio::spawn(async move {
            let media_semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests as usize));
            let mut crawled_urls = std::collections::HashSet::new();

            while let Some(page) = page_proc_rx.recv().await {
                let url_string = page.url.to_string();
                let filename_base = url_to_filename(&page.url);
                let file_path = html_dir.join(format!("{}.html", filename_base));

                // Save HTML content
                match File::create(&file_path).await {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(page.body.as_bytes()).await {
                            error!("Failed to write HTML content for {}: {}", url_string, e);
                            continue;
                        }

                        debug!("Saved HTML: {}", url_string);
                        crawled_urls.insert(url_string.clone());

                        // Extract and download images in a separate task
                        let url_clone = page.url.clone();
                        let body_clone = page.body.clone();
                        let media_dir_clone = media_dir.clone();
                        let client_clone = client.clone();
                        let semaphore_clone = Arc::clone(&media_semaphore);
                        let delay = config.delay_between_request_in_ms;

                        tokio::spawn(async move {
                            // Acquire semaphore permit
                            let permit = match semaphore_clone.acquire().await {
                                Ok(p) => p,
                                Err(_) => {
                                    error!("Failed to acquire semaphore permit for image download");
                                    return;
                                }
                            };

                            if let Err(e) = download_images(
                                &url_clone,
                                &body_clone,
                                &client_clone,
                                &media_dir_clone,
                                delay,
                            )
                            .await
                            {
                                warn!("Failed to download images for {}: {}", url_clone, e);
                            }
                            drop(permit); // Release permit
                        });
                    }
                    Err(e) => {
                        error!("Failed to create HTML file for {}: {}", url_string, e);
                    }
                }
            }

            crawled_urls
        });

        // Start crawling in a separate task
        let crawl_handle = tokio::spawn(async move {
            website.crawl().await;
            website // Return website to get stats later if needed
        });

        // Process pages received from the crawler
        while let Ok(page_data) = rx.recv().await {
            if let Some(bytes) = page_data.get_bytes() {
                match String::from_utf8(bytes.to_vec()) {
                    Ok(body) => {
                        let page = Page {
                            url: Url::parse(page_data.get_url())
                                .context("Invalid URL from spider")?,
                            body,
                        };
                        if let Err(e) = page_proc_tx.send(page).await {
                            error!("Failed to send page for processing: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to decode page content for {}: {}",
                            page_data.get_url(),
                            e
                        );
                    }
                }
            } else {
                warn!("Received page without content: {}", page_data.get_url());
            }
        }

        // Ensure the sender is dropped so the receiver task can finish
        drop(page_proc_tx);

        // Wait for crawling and processing to complete
        let crawl_result = crawl_handle.await;
        let download_result = download_task.await;

        if let Err(e) = crawl_result {
            error!("Crawler task failed for {}: {}", entry.name, e);
            // Optionally update entry status to Failed here or rely on retry logic
            return Err(anyhow::anyhow!("Crawler task failed: {}", e));
        }

        match download_result {
            Ok(crawled_urls) => {
                info!(
                    "Successfully processed {} URLs for entry: {}",
                    crawled_urls.len(),
                    entry.name
                );
                entry.last_download = Some(Utc::now());
                entry.version += 1;
                entry.status = CrawlStatus::Enabled; // Mark as success if crawl/download finishes
            }
            Err(e) => {
                error!("Download/Processing task failed for {}: {}", entry.name, e);
                entry.status = CrawlStatus::Failed; // Mark as failed if download task panics
                return Err(anyhow::anyhow!("Download/Processing task failed: {}", e));
            }
        }

        Ok(())
    }
}

/// Download images from HTML content
///
/// # Arguments
/// * `url` - The base URL for resolving relative links
/// * `html` - The HTML content to extract image URLs from
/// * `client` - The HTTP client to use
/// * `media_dir` - Directory to save media files
/// * `delay` - Delay between requests in milliseconds
///
/// # Returns
/// Ok(()) if successful
///
/// # Errors
/// Returns an error if images cannot be extracted or downloaded
async fn download_images(
    url: &Url,
    html: &str,
    client: &Client,
    media_dir: &Path,
    delay: u64,
) -> Result<()> {
    // Extract image URLs from HTML
    let mut image_urls = Vec::new();

    // Extract img src attributes (better parsing than before)
    for line in html.lines() {
        if line.contains("<img") && line.contains("src=") {
            // Handle src="..." format
            if let Some(start) = line.find("src=\"") {
                if let Some(end) = line[start + 5..].find('"') {
                    let src = &line[start + 5..start + 5 + end];
                    image_urls.push(src);
                }
            }
            // Handle src='...' format
            else if let Some(start) = line.find("src='") {
                if let Some(end) = line[start + 5..].find('\'') {
                    let src = &line[start + 5..start + 5 + end];
                    image_urls.push(src);
                }
            }
            // Handle src=... format without quotes
            else if let Some(start) = line.find("src=") {
                let src_part = &line[start + 4..];
                if let Some(end) = src_part.find(|c: char| c.is_whitespace() || c == '>') {
                    let src = &src_part[..end];
                    image_urls.push(src);
                }
            }
        }
    }

    // Download each image
    for image_url in image_urls {
        // Resolve relative URLs
        let full_url = match Url::parse(image_url) {
            Ok(url) => url,
            Err(_) => {
                // Handle relative URLs
                match url.join(image_url) {
                    Ok(url) => url,
                    Err(e) => {
                        warn!(
                            "Failed to parse/join image URL '{}' relative to '{}': {}",
                            image_url, url, e
                        );
                        continue;
                    }
                }
            }
        };

        // Download image file
        let filename_base = url_to_filename(&full_url);

        // Determine file extension
        let extension = full_url
            .path_segments()
            .and_then(|segs| segs.last())
            .and_then(|last_seg| last_seg.split('.').last())
            .unwrap_or("jpg"); // Default to jpg if no extension found

        let file_path = media_dir.join(format!("{}.{}", filename_base, extension));

        // Skip if already exists
        if file_path.exists() {
            debug!("Skipping existing image: {}", full_url);
            continue;
        }

        // Wait before making the request
        sleep(Duration::from_millis(delay)).await;

        // Download image
        match client.get(full_url.as_str()).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!(
                        "Failed to download image {} - Status: {}",
                        full_url,
                        response.status()
                    );
                    continue;
                }

                // Check if it's actually an image by content type
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                // Skip if not an image
                if !content_type.starts_with("image/") {
                    debug!(
                        "Skipping non-image content type '{}' for URL: {}",
                        content_type, full_url
                    );
                    continue;
                }

                match response.bytes().await {
                    Ok(bytes) => match File::create(&file_path).await {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(&bytes).await {
                                warn!("Failed to write image file {}: {}", file_path.display(), e);
                            } else {
                                debug!("Downloaded image: {}", full_url);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to create image file {}: {}", file_path.display(), e);
                        }
                    },
                    Err(e) => {
                        warn!(
                            "Failed to read image response bytes for {}: {}",
                            full_url, e
                        );
                    }
                }
            }
            Err(e) => {
                warn!("Failed to download image {}: {}", full_url, e);
            }
        }
    }

    Ok(())
}

/// Converts a URL to a valid filename, attempting to preserve structure.
///
/// # Arguments
/// * `url` - The URL to convert
///
/// # Returns
/// A string that can be used as a filename
fn url_to_filename(url: &Url) -> String {
    let host = url.host_str().unwrap_or("unknown_host");
    // Get path segments, filter out empty ones, join with underscores
    let path = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("_")
        })
        .unwrap_or_else(|| "".to_string()); // Use empty string if no path

    // Combine host and path
    let mut filename = if path.is_empty() {
        host.to_string()
    } else {
        format!("{}_{}", host, path)
    };

    // Replace definitely invalid characters for most filesystems
    filename = filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

    // Replace common web characters that might be problematic
    filename = filename
        .replace('&', "_and_")
        .replace('=', "_eq_")
        .replace('+', "_plus_");

    // Handle potential trailing characters or excessive underscores
    filename = filename.trim_matches('_').to_string();
    while filename.contains("__") {
        filename = filename.replace("__", "_");
    }

    // Limit length (e.g., 200 chars) to avoid filesystem limits
    let max_len = 200;
    if filename.len() > max_len {
        // Simple truncation, could be smarter (e.g., hash suffix)
        filename = filename.chars().take(max_len).collect();
        // Ensure it doesn't end with an underscore after truncation
        filename = filename.trim_end_matches('_').to_string();
    }

    // Handle case where filename might become empty after cleaning (e.g., URL was just "/")
    if filename.is_empty() {
        "index".to_string()
    } else {
        filename
    }
}
