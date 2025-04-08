//! # MI4ULINGS Docling
//!
//! A web crawler and document processor that downloads web pages,
//! converts them to Markdown, and processes the content.
//!
//! ## Features
//! - Crawls websites to a specified depth
//! - Downloads and saves web pages and media
//! - Converts HTML to Markdown using configurable methods
//! - Processes and combines content
//! - Robust error handling and retry logic

// Public modules
pub mod crawler;
pub mod converter;
pub mod processor;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use std::fs::create_dir_all;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use mi4ulings_config::{Config, Configuration};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use url::Url;

// Constants
const DEFAULT_RETRY_COUNT: u32 = 3;
const DEFAULT_DELAY_BETWEEN_REQUESTS_MS: u64 = 500;
const DEFAULT_MAX_CONCURRENT_REQUESTS: u32 = 1;
const DEFAULT_USER_AGENT: &str = "mi4uling-docling-bot";
const DEFAULT_REFETCH_DAYS: u32 = 100;
const DEFAULT_CRAWL_DEPTH: u32 = 1;

/// HTML to Markdown transformation method
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransformMethod {
    /// Use htmd library (default)
    Htmd,
    /// Use fast_html2md library
    FastHtml2md,
    /// Use Jina AI reader service
    JinaReader,
}

impl Default for TransformMethod {
    fn default() -> Self {
        TransformMethod::Htmd
    }
}

/// Status of a crawl task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CrawlStatus {
    /// Task is enabled and ready to run
    Enabled,
    /// Task is disabled by user
    Disabled,
    /// Task has failed and is halted
    Failed,
}

impl Default for CrawlStatus {
    fn default() -> Self {
        CrawlStatus::Enabled
    }
}

/// Configuration for the docling crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoclingConfig {
    /// Path where input files are stored
    pub inputs_path: PathBuf,
    /// Path where output files are stored
    pub outputs_path: PathBuf,
    /// Path where log files are stored
    pub logs_path: PathBuf,
    /// Suffix for HTML output directories
    pub output_parts_html_suffix: String,
    /// Suffix for media output directories
    pub output_parts_media_suffix: String,
    /// Suffix for Markdown output directories
    pub output_parts_markdown_suffix: String,
    /// Suffix for final Markdown result directories
    pub output_parts_markdown_results_suffix: String,
    /// Number of retry attempts for failed downloads
    pub retry_count: u32,
    /// Delay between HTTP requests in milliseconds
    pub delay_between_request_in_ms: u64,
    /// Maximum number of concurrent requests
    pub max_concurrent_requests: u32,
    /// User agent string for HTTP requests
    pub user_agent: String,
    /// Whether to respect robots.txt
    pub respect_robots_txt: bool,
    /// Method to use for HTML to Markdown transformation
    pub transform_md_using: TransformMethod,
    /// Delays between retry attempts in seconds
    pub retry_delay: Vec<u64>,
    /// Number of days after which to re-fetch content
    pub refetch_after_days: u32,
    /// Default crawl depth
    pub default_deep: u32,
}

impl Default for DoclingConfig {
    fn default() -> Self {
        Self {
            inputs_path: PathBuf::from("inputs"),
            outputs_path: PathBuf::from("outputs"),
            logs_path: PathBuf::from("logs"),
            output_parts_html_suffix: "parts_html".to_string(),
            output_parts_media_suffix: "parts_media".to_string(),
            output_parts_markdown_suffix: "parts_md".to_string(),
            output_parts_markdown_results_suffix: "results_md".to_string(),
            retry_count: DEFAULT_RETRY_COUNT,
            delay_between_request_in_ms: DEFAULT_DELAY_BETWEEN_REQUESTS_MS,
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            respect_robots_txt: true,
            transform_md_using: TransformMethod::default(),
            retry_delay: vec![10, 40, 200],
            refetch_after_days: DEFAULT_REFETCH_DAYS,
            default_deep: DEFAULT_CRAWL_DEPTH,
        }
    }
}

impl Configuration for DoclingConfig {
    fn crate_name() -> &'static str {
        "mi4ulings-docling"
    }
}

/// URL entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlEntry {
    /// URL to crawl
    pub url: String,
    /// Name of the entry (used for file naming)
    pub name: String,
    /// Last successful download date and time
    pub last_download: Option<DateTime<Utc>>,
    /// Last attempt date and time
    pub last_try: Option<DateTime<Utc>>,
    /// Last failure date and time
    pub last_fail: Option<DateTime<Utc>>,
    /// How deep to crawl (number of link levels)
    pub crawl_depth: u32,
    /// Current status of the entry
    pub status: CrawlStatus,
    /// Version of the entry
    pub version: u32,
}

impl UrlEntry {
    /// Create a new URL entry
    pub fn new(url: &str, name: &str, crawl_depth: Option<u32>) -> Result<Self> {
        // Validate URL
        let parsed_url = Url::parse(url).context("Invalid URL format")?;
        
        Ok(Self {
            url: parsed_url.to_string(),
            name: name.to_string(),
            last_download: None,
            last_try: None,
            last_fail: None,
            crawl_depth: crawl_depth.unwrap_or(DEFAULT_CRAWL_DEPTH),
            status: CrawlStatus::Enabled,
            version: 1,
        })
    }
    
    /// Check if the entry should be refreshed based on the last download date
    pub fn should_refresh(&self, config: &DoclingConfig) -> bool {
        if let Some(last_download) = self.last_download {
            let now = Utc::now();
            let duration = now.signed_duration_since(last_download);
            let days = duration.num_days();
            days >= config.refetch_after_days as i64
        } else {
            true
        }
    }
}

/// Collection of URL entries
#[derive(Debug, Serialize, Deserialize)]
pub struct UrlEntries {
    /// Map of URL entries by name
    pub entries: HashMap<String, UrlEntry>,
}

impl Default for UrlEntries {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

/// Store URL entries in a TOML file
pub fn save_entries(entries: &UrlEntries) -> Result<()> {
    let config = Config::<DoclingConfig>::load_or_default()?;
    let inputs_path = config.data.inputs_path.clone();
    
    std::fs::create_dir_all(&inputs_path)
        .context("Failed to create inputs directory")?;
    
    let entries_path = inputs_path.join("entries.toml");
    let toml_string = toml::to_string(entries)
        .context("Failed to serialize URL entries to TOML")?;
    
    std::fs::write(&entries_path, toml_string)
        .context("Failed to write URL entries to file")?;
    
    Ok(())
}

/// Load URL entries from a TOML file
pub fn load_entries() -> Result<UrlEntries> {
    let config = Config::<DoclingConfig>::load_or_default()?;
    let inputs_path = config.data.inputs_path.clone();
    let entries_path = inputs_path.join("entries.toml");
    
    if !entries_path.exists() {
        return Ok(UrlEntries::default());
    }
    
    let toml_string = std::fs::read_to_string(&entries_path)
        .context("Failed to read URL entries from file")?;
    
    let entries: UrlEntries = toml::from_str(&toml_string)
        .context("Failed to parse URL entries from TOML")?;
    
    Ok(entries)
}

/// Add a new URL entry
pub fn add_url(url: &str, name_opt: Option<&str>, crawl_depth: Option<u32>) -> Result<()> {
    // Generate name from URL if not provided
    let name = match name_opt {
        Some(n) => n.to_string(),
        None => {
            let parsed_url = Url::parse(url).context("Invalid URL format")?;
            parsed_url.host_str()
                .map(|h| h.to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        }
    };
    
    // Create new entry
    let entry = UrlEntry::new(url, &name, crawl_depth)?;
    
    // Load existing entries
    let mut entries = load_entries()?;
    
    // Check if name already exists
    if entries.entries.contains_key(&name) {
        return Err(anyhow::anyhow!("Entry with name '{}' already exists", name));
    }
    
    // Add new entry
    entries.entries.insert(name.clone(), entry);
    
    // Save entries
    save_entries(&entries)?;
    
    info!("Added URL entry: {} ({})", name, url);
    Ok(())
}

/// Remove a URL entry
pub fn remove_url(name: &str) -> Result<()> {
    // Load existing entries
    let mut entries = load_entries()?;
    
    // Check if entry exists
    if !entries.entries.contains_key(name) {
        return Err(anyhow::anyhow!("Entry with name '{}' does not exist", name));
    }
    
    // Remove entry
    entries.entries.remove(name);
    
    // Save entries
    save_entries(&entries)?;
    
    info!("Removed URL entry: {}", name);
    Ok(())
}

/// Stop a URL entry (disable it)
pub fn stop_url(name: &str) -> Result<()> {
    // Load existing entries
    let mut entries = load_entries()?;
    
    // Check if entry exists
    let entry = entries.entries.get_mut(name)
        .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name))?;
    
    // Update status
    entry.status = CrawlStatus::Disabled;
    
    // Save entries
    save_entries(&entries)?;
    
    info!("Stopped URL entry: {}", name);
    Ok(())
}

/// Start a URL entry (enable it)
pub fn start_url(name: &str) -> Result<()> {
    // Load existing entries
    let mut entries = load_entries()?;
    
    // Check if entry exists
    let entry = entries.entries.get_mut(name)
        .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name))?;
    
    // Update status
    entry.status = CrawlStatus::Enabled;
    
    // Save entries
    save_entries(&entries)?;
    
    info!("Started URL entry: {}", name);
    Ok(())
}

/// List all URL entries
pub fn list_urls() -> Result<Vec<UrlEntry>> {
    let entries = load_entries()?;
    Ok(entries.entries.values().cloned().collect())
}

/// Runs the crawling, conversion, and processing for a URL entry
/// 
/// This is the main function that orchestrates the entire process:
/// 1. Crawls the website to the specified depth
/// 2. Downloads HTML and media files
/// 3. Converts HTML to Markdown
/// 4. Processes and combines Markdown files
/// 5. Creates the final output file
pub async fn run_entry(name: &str) -> Result<PathBuf> {
    // Load configuration
    let config = Config::<DoclingConfig>::load_or_default()?;
    let config_data = config.data.clone();
    
    // Load entries
    let mut entries = load_entries()?;
    
    // Check if entry exists and is enabled before processing
    {
        let entry = entries.entries.get(name)
            .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name))?;
        
        if entry.status == CrawlStatus::Disabled {
            return Err(anyhow::anyhow!("Entry '{}' is disabled", name));
        }
    }
    
    // Create directories (not dependent on entry borrow)
    let base_dir = config_data.outputs_path.join(name);
    create_dir_all(&base_dir).context("Failed to create output directory")?;
    
    // Create error directory 
    let error_dir = base_dir.join("ERRORS");
    create_dir_all(&error_dir).context("Failed to create error directory")?;
    
    // Crawl and download
    let mut retry_count = 0;
    let mut success = false;
    let mut last_error = None;
    
    while retry_count < config_data.retry_count && !success {
        // Clone the name since we'll use it in multiple places
        let name_clone = name.to_string();
        
        // Process with isolated scope to limit borrow duration
        let process_result = {
            // Get mutable reference to the entry within this scope
            let entry = entries.entries.get_mut(&name_clone)
                .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name_clone))?;
            
            // Process entry without holding the borrow outside this scope
            process_with_retry(entry, &config_data).await
        };
        
        match process_result {
            Ok(result_file) => {
                // Update entry in a separate scope
                {
                    let entry = entries.entries.get_mut(name)
                        .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name))?;
                    
                    entry.last_download = Some(Utc::now());
                    entry.status = CrawlStatus::Enabled;
                    entry.version += 1;
                }
                
                // Now save entries after the borrow is released
                save_entries(&entries)?;
                
                success = true;
                return Ok(result_file);
            }
            Err(e) => {
                // Log error
                let error_message = format!("Error processing '{}': {}", name, e);
                error!("{}", error_message);
                
                // Save error to file
                let now = Utc::now();
                let error_file = error_dir.join(format!("error_{}.txt", now.format("%Y%m%d_%H%M%S")));
                std::fs::write(&error_file, error_message)
                    .context("Failed to write error file")?;
                
                // Update entry in a separate scope
                {
                    let entry = entries.entries.get_mut(name)
                        .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name))?;
                    
                    entry.last_fail = Some(now);
                }
                
                // Now save entries after the borrow is released
                save_entries(&entries)?;
                
                // Get retry delay
                let delay = if retry_count < config_data.retry_delay.len() as u32 {
                    config_data.retry_delay[retry_count as usize]
                } else {
                    60 // Default to 60 seconds if no specific delay is configured
                };
                
                // Wait before retrying
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                
                retry_count += 1;
                last_error = Some(e);
            }
        }
    }
    
    // If we get here, all retries failed
    {
        // Get entry in a new scope to limit borrow duration
        let entry = entries.entries.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Entry with name '{}' does not exist", name))?;
        entry.status = CrawlStatus::Failed;
    }
    
    // Save entries after the borrow is released
    save_entries(&entries)?;
    
    Err(anyhow::anyhow!("Failed to process entry after {} retries: {}", 
                      retry_count, 
                      last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error"))))
}

/// Process a URL entry with retry logic
async fn process_with_retry(entry: &mut UrlEntry, config: &DoclingConfig) -> Result<PathBuf> {
    // Initialize components
    let mut crawler = crawler::Crawler::new(config.clone())?;
    let converter = converter::Converter::new(config.clone())?;
    let processor = processor::Processor::new(config.clone());
    
    // Step 1: Crawl and download
    crawler.process_entry(entry).await?;
    
    // Step 2: Convert HTML to Markdown
    let md_files = converter.convert_directory(&entry.name).await?;
    
    // Step 3: Process and combine Markdown files
    let result_file = processor.process_entry(&entry.name, &entry.url)?;
    
    Ok(result_file)
}