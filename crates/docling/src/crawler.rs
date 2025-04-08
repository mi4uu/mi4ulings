//! Web crawler functionality for downloading web pages and extracting links
//!
//! This module provides functionality to:
//! - Crawl websites to a specified depth
//! - Download and save HTML content
//! - Extract and download media files (images)
//! - Support configurable parameters like crawl depth, delay, and user agent
//! - Handle URL normalization and conversion to filenames
//! - Process URLs concurrently with tokio async/await
//!
//! The main entry point is the `Crawler` struct, which orchestrates the entire
//! crawling process for a given URL entry.

use std::collections::{HashSet, VecDeque};
use std::fs::create_dir_all;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use spider::website::Website;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, mpsc, Semaphore};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::{DoclingConfig, UrlEntry, CrawlStatus};

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

/// Enhanced Spider implementation that leverages tokio for concurrent processing
///
/// This provides a more robust interface for crawling websites using tokio's
/// concurrency primitives like broadcast channels and spawn.
struct AsyncSpider {
    /// Base URL for crawling
    base_url: Url,
    /// HTTP client for making requests
    client: Client,
    /// Delay between requests in milliseconds
    delay: u64,
    /// Maximum crawl depth
    max_depth: usize,
    /// Whether to respect robots.txt
    respect_robots: bool,
    /// Discovered links
    links: Arc<Mutex<HashSet<String>>>,
    /// Links to be processed
    queue: Arc<Mutex<VecDeque<(String, usize)>>>,
    /// Set of visited URLs
    visited: Arc<Mutex<HashSet<String>>>,
    /// Broadcast sender for pages
    page_tx: broadcast::Sender<Page>,
    /// Semaphore for limiting concurrent requests
    semaphore: Arc<Semaphore>,
}

impl AsyncSpider {
    /// Create a new AsyncSpider with the specified website configuration
    ///
    /// # Arguments
    /// * `url` - The starting URL for crawling
    /// * `config` - The configuration for crawling
    ///
    /// # Returns
    /// A new AsyncSpider instance and broadcast receiver for pages
    ///
    /// # Errors
    /// Returns an error if the URL is invalid
    fn new(url: &str, config: &DoclingConfig) -> Result<(Self, broadcast::Receiver<Page>)> {
        let base_url = Url::parse(url).context("Invalid URL format")?;
        
        // Create HTTP client with user agent and other settings
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        let links = Arc::new(Mutex::new(HashSet::new()));
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let visited = Arc::new(Mutex::new(HashSet::new()));
        
        // Initialize with the starting URL at depth 0
        queue.lock().unwrap().push_back((url.to_string(), 0));
        
        // Create broadcast channel for pages
        let (page_tx, page_rx) = broadcast::channel(100);
        
        // Create semaphore for limiting concurrent requests
        let max_concurrent = config.max_concurrent_requests.max(1) as usize;
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        
        Ok((
            Self {
                base_url,
                client,
                delay: config.delay_between_request_in_ms,
                max_depth: config.default_deep as usize,
                respect_robots: config.respect_robots_txt,
                links,
                queue,
                visited,
                page_tx,
                semaphore,
            },
            page_rx
        ))
    }
    
    /// Start crawling asynchronously
    ///
    /// This launches multiple worker tasks that fetch pages concurrently,
    /// respecting the maximum concurrent requests setting.
    ///
    /// # Returns
    /// A future that resolves when crawling is complete
    pub async fn crawl(&self) -> Result<()> {
        // Create a channel for worker completion signals
        let (done_tx, mut done_rx) = mpsc::channel(1);
        
        // Clone references for worker tasks
        let queue = Arc::clone(&self.queue);
        let visited = Arc::clone(&self.visited);
        let links = Arc::clone(&self.links);
        let semaphore = Arc::clone(&self.semaphore);
        let page_tx = self.page_tx.clone();
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let delay = self.delay;
        let max_depth = self.max_depth;
        
        // Launch the crawler task
        tokio::spawn(async move {
            loop {
                // Get the next URL from the queue
                let next_item = {
                    let mut queue = queue.lock().unwrap();
                    queue.pop_front()
                };
                
                match next_item {
                    Some((url, depth)) => {
                        // Skip if already visited
                        {
                            let visited_urls = visited.lock().unwrap();
                            if visited_urls.contains(&url) {
                                continue;
                            }
                        }
                        
                        // Acquire semaphore permit to limit concurrent requests
                        let permit = semaphore.clone().acquire_owned().await.unwrap();
                        
                        // Clone references for the task
                        let queue = Arc::clone(&queue);
                        let visited = Arc::clone(&visited);
                        let links = Arc::clone(&links);
                        let page_tx = page_tx.clone();
                        let client = client.clone();
                        let base_url = base_url.clone();
                        
                        // Process URL in a new task
                        tokio::spawn(async move {
                            // Mark as visited
                            {
                                let mut visited_urls = visited.lock().unwrap();
                                visited_urls.insert(url.clone());
                            }
                            
                            // Fetch the URL
                            match fetch_url(&client, &url).await {
                                Ok((parsed_url, body)) => {
                                    // Extract links if not at max depth
                                    if depth < max_depth {
                                        let new_links = extract_links(&body, &parsed_url);
                                        
                                        // Add new links to queue
                                        {
                                            let mut link_set = links.lock().unwrap();
                                            let mut queue = queue.lock().unwrap();
                                            let visited_urls = visited.lock().unwrap();
                                            
                                            for link in new_links {
                                                // Skip if already visited or queued
                                                if visited_urls.contains(&link) || link_set.contains(&link) {
                                                    continue;
                                                }
                                                
                                                // Check if link is in the same domain
                                                match Url::parse(&link) {
                                                    Ok(parsed_link) => {
                                                        if parsed_link.host() == base_url.host() {
                                                            link_set.insert(link.clone());
                                                            queue.push_back((link, depth + 1));
                                                        }
                                                    },
                                                    Err(_) => continue, // Skip invalid URLs
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Send the page to the channel
                                    let page = Page { url: parsed_url, body };
                                    let _ = page_tx.send(page);
                                    
                                    // Wait for rate limiting
                                    sleep(Duration::from_millis(delay)).await;
                                },
                                Err(e) => {
                                    warn!("Failed to fetch URL {}: {}", url, e);
                                }
                            }
                            
                            // Release the semaphore permit
                            drop(permit);
                        });
                    },
                    None => {
                        // Queue is empty, check if all workers are done
                        let active_count = Arc::strong_count(&semaphore) - 1; // Minus self
                        let permits_available = semaphore.available_permits();
                        
                        if active_count == permits_available {
                            // All workers are idle, crawling is done
                            let _ = done_tx.send(()).await;
                            break;
                        }
                        
                        // Wait a bit and check again
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        // Wait for crawling to complete
        let _ = done_rx.recv().await;
        
        Ok(())
    }
}

/// Fetch a URL and return its content
///
/// # Arguments
/// * `client` - The HTTP client to use
/// * `url` - The URL to fetch
///
/// # Returns
/// A tuple of (parsed URL, HTML content) if successful
///
/// # Errors
/// Returns an error if the URL is invalid, the request fails, or the
/// response cannot be read
async fn fetch_url(client: &Client, url: &str) -> Result<(Url, String)> {
    let parsed_url = Url::parse(url).context("Invalid URL")?;
    let response = client.get(url).send().await
        .context("Failed to fetch URL")?;
    let body = response.text().await
        .context("Failed to read response body")?;
    Ok((parsed_url, body))
}

/// Extract links from HTML content
///
/// # Arguments
/// * `html` - The HTML content to extract links from
/// * `base_url` - The base URL for resolving relative links
///
/// # Returns
/// A vector of absolute URLs found in the HTML
fn extract_links(html: &str, base_url: &Url) -> Vec<String> {
    let mut links = Vec::new();
    
    // Extract href attributes
    for line in html.lines() {
        if line.contains("href=\"") {
            if let Some(start) = line.find("href=\"") {
                if let Some(end) = line[start + 6..].find('"') {
                    let href = &line[start + 6..start + 6 + end];
                    
                    // Skip fragment-only, javascript, and mailto links
                    if href.starts_with('#') || 
                       href.starts_with("javascript:") || 
                       href.starts_with("mailto:") {
                        continue;
                    }
                    
                    // Resolve relative URLs
                    match base_url.join(href) {
                        Ok(full_url) => {
                            links.push(full_url.to_string());
                        },
                        Err(_) => continue,
                    }
                }
            }
        }
    }
    
    links
}

/// Crawler handles web crawling and content downloading
///
/// This is the main component that orchestrates the crawling process:
/// 1. Configures and initializes a spider for a URL entry
/// 2. Crawls the website to the specified depth
/// 3. Downloads and saves HTML content
/// 4. Extracts and downloads media files (images)
/// 5. Updates the URL entry with status information
pub struct Crawler {
    /// Configuration for the crawler
    config: DoclingConfig,
    /// HTTP client for making requests
    client: Client,
    /// Set of visited URLs to avoid duplicates
    visited: HashSet<String>,
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
        // Create HTTP client with user agent and other settings
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self {
            config,
            client,
            visited: HashSet::new(),
        })
    }
    
    /// Processes a URL entry, downloading content and finding links
    ///
    /// This is the main entry point for crawling a website. It:
    /// 1. Updates the entry's status
    /// 2. Creates necessary output directories
    /// 3. Configures and starts the crawler
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
        
        // Create AsyncSpider and start crawling
        let (spider, mut pages_rx) = AsyncSpider::new(&entry.url, &self.config)?;
        
        // Launch the spider in a separate task
        let spider_task = tokio::spawn(async move {
            spider.crawl().await
        });
        
        // Create a channel for downloaded pages
        let (dl_tx, mut dl_rx) = mpsc::channel::<(Url, String)>(100);
        
        // Process downloaded pages
        let html_dir = html_output_dir.clone();
        let media_dir = media_output_dir.clone();
        let client = self.client.clone();
        let config = self.config.clone();
        let dl_task = tokio::spawn(async move {
            // Create semaphore for limiting concurrent media downloads
            let media_semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests as usize));
            let mut crawled_urls = HashSet::new();
            
            while let Some((url, body)) = dl_rx.recv().await {
                let file_path = html_dir.join(format!("{}.html", url_to_filename(&url)));
                
                // Save HTML content
                match File::create(&file_path).await {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(body.as_bytes()).await {
                            error!("Failed to write HTML content: {}", e);
                            continue;
                        }
                        
                        debug!("Downloaded: {}", url);
                        crawled_urls.insert(url.to_string());
                        
                        // Extract and download images
                        // Use a separate task to avoid blocking
                        let url_clone = url.clone();
                        let body_clone = body.clone();
                        let media_dir_clone = media_dir.clone();
                        let client_clone = client.clone();
                        let semaphore_clone = Arc::clone(&media_semaphore);
                        let delay = config.delay_between_request_in_ms;
                        
                        tokio::spawn(async move {
                            // Acquire semaphore permit
                            let _permit = semaphore_clone.acquire().await.unwrap();
                            
                            if let Err(e) = download_images(&url_clone, &body_clone, &client_clone, &media_dir_clone, delay).await {
                                warn!("Failed to download images for {}: {}", url_clone, e);
                            }
                        });
                    },
                    Err(e) => {
                        error!("Failed to create HTML file: {}", e);
                    }
                }
            }
            
            crawled_urls
        });
        
        // Receive pages from spider and send them to the download task
        let mut page_count = 0;
        while let Ok(page) = pages_rx.recv().await {
            let _ = dl_tx.send((page.url, page.body)).await;
            page_count += 1;
        }
        
        // Wait for download task to complete
        match dl_task.await {
            Ok(crawled_urls) => {
                info!("Processed {} URLs for entry: {}", crawled_urls.len(), entry.name);
            },
            Err(e) => {
                error!("Download task failed: {}", e);
            }
        }
        
        // Update entry status
        entry.last_download = Some(Utc::now());
        entry.version += 1;
        
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
    delay: u64
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
                        warn!("Failed to parse image URL {}: {}", image_url, e);
                        continue;
                    }
                }
            }
        };
        
        // Download image file
        let filename = url_to_filename(&full_url);
        
        // Determine file extension
        let extension = full_url.path().split('.').last().unwrap_or("jpg");
        let file_path = media_dir.join(format!("{}.{}", filename, extension));
        
        // Skip if already exists
        if file_path.exists() {
            continue;
        }
        
        // Download image
        match client.get(full_url.as_str()).send().await {
            Ok(response) => {
                // Check if it's actually an image by content type
                let content_type = response.headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                
                // Skip if not an image
                if !content_type.starts_with("image/") {
                    continue;
                }
                
                match response.bytes().await {
                    Ok(bytes) => {
                        match File::create(&file_path).await {
                            Ok(mut file) => {
                                if let Err(e) = file.write_all(&bytes).await {
                                    warn!("Failed to write image file: {}", e);
                                } else {
                                    debug!("Downloaded image: {}", full_url);
                                }
                            },
                            Err(e) => {
                                warn!("Failed to create image file: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read image response: {}", e);
                    }
                }
            },
            Err(e) => {
                warn!("Failed to download image {}: {}", full_url, e);
            }
        }
        
        // Wait between requests
        sleep(Duration::from_millis(delay)).await;
    }
    
    Ok(())
}

/// Converts a URL to a valid filename
///
/// # Arguments
/// * `url` - The URL to convert
///
/// # Returns
/// A string that can be used as a filename
fn url_to_filename(url: &Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    let path = url.path().trim_matches('/');
    
    // Combine host and path, replace invalid characters
    let mut filename = format!("{}_{}", host, path);
    
    // Replace invalid characters
    filename = filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '], "_");
    
    // Limit length
    if filename.len() > 100 {
        filename = filename.chars().take(100).collect();
    }
    
    filename
}