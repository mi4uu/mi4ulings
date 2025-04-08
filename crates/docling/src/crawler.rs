//! Web crawler functionality for downloading web pages and extracting links

use std::collections::HashSet;
use std::fs::create_dir_all;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use spider::website::Website;
// Custom Spider implementation since spider crate doesn't have Spider in root
// Page returned by Spider.next()
struct Page {
    url: Url,
    body: String,
}

// Custom Spider implementation that wraps Website
struct Spider {
    website: Website,
    client: Client,
    delay: u64,
}

impl Spider {
    fn new(website: Website) -> Self {
        Self {
            website,
            client: Client::new(),
            delay: 500, // Default delay between requests
        }
    }
    
    async fn next(&mut self) -> Option<Page> {
        // Simplified implementation: just crawl the initial URL
        // A real implementation would iterate through links
        if self.website.get_links().is_empty() {
            // First page, fetch the initial URL
            match self.fetch_url(self.website.get_url()).await {
                Ok((url, body)) => {
                    // Process links and add them to website
                    self.website.crawl().await;
                    Some(Page { url, body })
                }
                Err(_) => None,
            }
        } else {
            // No more pages
            None
        }
    }
    
    async fn fetch_url(&self, url: &str) -> Result<(Url, String)> {
        let parsed_url = Url::parse(url).context("Invalid URL")?;
        let response = self.client.get(url).send().await
            .context("Failed to fetch URL")?;
        let body = response.text().await
            .context("Failed to read response body")?;
        Ok((parsed_url, body))
    }
}
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::{DoclingConfig, UrlEntry, CrawlStatus};

/// Crawler handles the web crawling and content downloading
pub struct Crawler {
    /// Configuration for the crawler
    config: DoclingConfig,
    /// HTTP client for making requests
    client: Client,
    /// Set of visited URLs to avoid duplicates
    visited: HashSet<String>,
}

impl Crawler {
    /// Create a new crawler with the given configuration
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
    
    /// Process a URL entry, downloading content and finding links
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
        
        // Initialize spider with config options
        let spider_config = self.create_spider_config(&entry.url, entry.crawl_depth)?;
        
        // Start crawling
        let crawled_urls = self.crawl(spider_config, &html_output_dir, &media_output_dir).await?;
        
        // Update entry status
        entry.last_download = Some(Utc::now());
        entry.version += 1;
        
        info!("Processed {} URLs for entry: {}", crawled_urls.len(), entry.name);
        
        Ok(())
    }
    
    // Create spider configuration
    fn create_spider_config(&self, url: &str, depth: u32) -> Result<Website> {
        let mut website = Website::new(url);
        website.with_respect_robots_txt(self.config.respect_robots_txt);
        website.with_subdomains(false);
        website.with_tld(false);
        website.with_delay(self.config.delay_between_request_in_ms);
        website.with_depth(depth as usize);
            
        Ok(website)
    }
    
    // Crawl a website and save content
    async fn crawl(
        &mut self, 
        config: Website,
        html_dir: &Path,
        media_dir: &Path
    ) -> Result<Vec<String>> {
        let mut spider = Spider::new(config);
        let mut crawled_urls = Vec::new();
        
        // Process each page
        while let Some(page) = spider.next().await {
            let url = page.url.to_string();
            
            // Skip if already visited
            if self.visited.contains(&url) {
                continue;
            }
            
            self.visited.insert(url.clone());
            crawled_urls.push(url.clone());
            
            // Download content
            match self.download_page(&page.url, page.body, html_dir, media_dir).await {
                Ok(_) => {
                    debug!("Downloaded: {}", url);
                    // Wait between requests
                    sleep(Duration::from_millis(self.config.delay_between_request_in_ms)).await;
                }
                Err(e) => {
                    error!("Failed to download {}: {}", url, e);
                }
            }
        }
        
        Ok(crawled_urls)
    }
    
    // Download a page and save it
    async fn download_page(
        &mut self,
        url: &Url,
        body: String,
        html_dir: &Path,
        media_dir: &Path
    ) -> Result<()> {
        // Generate filename from URL
        let filename = self.url_to_filename(url);
        let file_path = html_dir.join(format!("{}.html", filename));
        
        // Create file and write content
        let mut file = File::create(&file_path).await
            .context("Failed to create HTML file")?;
        
        file.write_all(body.as_bytes()).await
            .context("Failed to write HTML content")?;
        
        // Extract and download media files
        self.download_media(url, &body, media_dir).await?;
        
        Ok(())
    }
    
    // Extract and download media files
    async fn download_media(&mut self, base_url: &Url, html: &str, media_dir: &Path) -> Result<()> {
        // Extract image URLs from HTML
        // This is a simplified implementation - a real implementation would use an HTML parser
        let mut media_urls = Vec::new();
        
        // Extract img src attributes
        for line in html.lines() {
            if line.contains("<img") && line.contains("src=") {
                if let Some(start) = line.find("src=\"") {
                    if let Some(end) = line[start + 5..].find('"') {
                        let src = &line[start + 5..start + 5 + end];
                        media_urls.push(src);
                    }
                }
            }
        }
        
        // Download each media file
        for media_url in media_urls {
            // Resolve relative URLs
            let full_url = match Url::parse(media_url) {
                Ok(url) => url,
                Err(_) => {
                    // Handle relative URLs
                    match base_url.join(media_url) {
                        Ok(url) => url,
                        Err(e) => {
                            warn!("Failed to parse media URL {}: {}", media_url, e);
                            continue;
                        }
                    }
                }
            };
            
            // Skip if already visited
            if self.visited.contains(&full_url.to_string()) {
                continue;
            }
            
            self.visited.insert(full_url.to_string());
            
            // Download media file
            match self.download_media_file(&full_url, media_dir).await {
                Ok(_) => {
                    debug!("Downloaded media: {}", full_url);
                    // Wait between requests
                    sleep(Duration::from_millis(self.config.delay_between_request_in_ms)).await;
                }
                Err(e) => {
                    warn!("Failed to download media {}: {}", full_url, e);
                }
            }
        }
        
        Ok(())
    }
    
    // Download a media file
    async fn download_media_file(&self, url: &Url, media_dir: &Path) -> Result<()> {
        // Generate filename from URL
        let filename = self.url_to_filename(url);
        
        // Determine file extension
        let extension = url.path().split('.').last().unwrap_or("bin");
        let file_path = media_dir.join(format!("{}.{}", filename, extension));
        
        // Download file
        let response = self.client.get(url.as_str()).send().await
            .context("Failed to download media file")?;
        
        let bytes = response.bytes().await
            .context("Failed to read media response")?;
        
        // Create file and write content
        let mut file = File::create(&file_path).await
            .context("Failed to create media file")?;
        
        file.write_all(&bytes).await
            .context("Failed to write media content")?;
        
        Ok(())
    }
    
    // Convert URL to a valid filename
    fn url_to_filename(&self, url: &Url) -> String {
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
}