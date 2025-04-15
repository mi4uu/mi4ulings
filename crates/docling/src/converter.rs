//! Converter for transforming HTML content to Markdown
//! Supports multiple conversion methods: htmd, fast_html2md, and jina_reader

use std::fs::{self, create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
#[cfg(not(any(feature = "htmd", feature = "fast-html2md")))]
use regex::Regex; // Only needed if neither feature is enabled
use reqwest::{Client, ClientBuilder};
use tracing::{debug, error, info, warn};

use crate::{DoclingConfig, TransformMethod};

/// Converter for HTML to Markdown transformation
pub struct Converter {
    /// Configuration for the converter
    config: DoclingConfig,
    /// HTTP client for making requests (used by Jina Reader)
    client: Client,
}

impl Converter {
    /// Create a new converter with the given configuration
    pub fn new(config: DoclingConfig) -> Result<Self> {
        // Set up HTTP client with proper timeouts to prevent hanging
        let client = ClientBuilder::new()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(60)) // 1 minute timeout for all requests
            .connect_timeout(Duration::from_secs(30)) // 30 seconds connect timeout
            .build()
            .context("Failed to create HTTP client")?;

        info!(
            "Converter initialized with {:?} transformation method",
            config.transform_md_using
        );

        Ok(Self { config, client })
    }

    /// Convert HTML files in a directory to Markdown
    pub async fn convert_directory(&self, entry_name: &str) -> Result<Vec<PathBuf>> {
        let start_time = Instant::now();
        info!(
            "Starting conversion of HTML files for entry '{}'",
            entry_name
        );

        let base_dir = self.config.outputs_path.join(entry_name);
        let html_dir = base_dir.join(&self.config.output_parts_html_suffix);
        let md_dir = base_dir.join(&self.config.output_parts_markdown_suffix);

        info!("HTML directory: {}", html_dir.display());
        info!("Markdown directory: {}", md_dir.display());

        // Create markdown directory if it doesn't exist
        create_dir_all(&md_dir).context("Failed to create Markdown output directory")?;

        // Get all HTML files
        info!("Scanning for HTML files in {}", html_dir.display());
        let html_files = fs::read_dir(&html_dir)
            .context("Failed to read HTML directory")?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                    && entry
                        .path()
                        .extension()
                        .map(|ext| ext == "html")
                        .unwrap_or(false)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();

        info!("Found {} HTML files to convert", html_files.len());

        let mut converted_files = Vec::new();
        let mut conversion_count = 0;
        let total_files = html_files.len();

        // Process each HTML file
        for html_file in html_files {
            conversion_count += 1;
            let filename = html_file.file_stem().unwrap().to_string_lossy();
            let md_file = md_dir.join(format!("{}.md", filename));

            info!(
                "Converting file {}/{}: {} -> {}",
                conversion_count,
                total_files,
                html_file.display(),
                md_file.display()
            );

            let file_start_time = Instant::now();
            match self.convert_file(&html_file, &md_file).await {
                Ok(_) => {
                    let duration = file_start_time.elapsed();
                    info!(
                        "Successfully converted {}/{} in {:.2?}",
                        conversion_count, total_files, duration
                    );
                    converted_files.push(md_file);
                }
                Err(e) => {
                    let duration = file_start_time.elapsed();
                    error!(
                        "Failed to convert file {}/{} after {:.2?}: {}",
                        conversion_count, total_files, duration, e
                    );
                }
            }
        }

        let total_duration = start_time.elapsed();
        info!(
            "Converted {}/{} HTML files to Markdown in {:.2?}",
            converted_files.len(),
            total_files,
            total_duration
        );

        Ok(converted_files)
    }

    /// Convert a single HTML file to Markdown
    pub async fn convert_file(&self, html_file: &Path, md_file: &Path) -> Result<()> {
        let start_time = Instant::now();
        info!("Starting conversion of file: {}", html_file.display());

        // Read HTML content
        debug!("Reading HTML content from: {}", html_file.display());
        let read_start = Instant::now();
        let html_content = read_to_string(html_file).context("Failed to read HTML file")?;
        let read_duration = read_start.elapsed();
        info!(
            "Read HTML content ({} bytes) in {:.2?}",
            html_content.len(),
            read_duration
        );

        // Convert to Markdown based on config
        info!(
            "Converting using {:?} method",
            self.config.transform_md_using
        );
        let convert_start = Instant::now();
        let markdown_result = match self.config.transform_md_using {
            TransformMethod::Htmd => {
                info!("Using htmd conversion method");
                self.convert_with_htmd(&html_content)
            }
            TransformMethod::FastHtml2md => {
                info!("Using fast_html2md conversion method");
                self.convert_with_fast_html2md(&html_content)
            }
            TransformMethod::JinaReader => {
                info!("Using Jina Reader conversion method");
                self.convert_with_jina_reader(html_file).await
            }
        };

        let markdown = match markdown_result {
            Ok(md) => md,
            Err(e) => {
                warn!(
                    "Conversion failed for {}: {}. Falling back to simple conversion.",
                    html_file.display(),
                    e
                );
                // Add title as header even in fallback
                let title = self.extract_title_from_html(&html_content);
                let fallback_md = self.simple_html_to_markdown(&html_content);
                if let Some(t) = title {
                    format!("# {}\n\n{}", t, fallback_md)
                } else {
                    fallback_md
                }
            }
        };

        let convert_duration = convert_start.elapsed();
        info!(
            "Conversion completed in {:.2?}, produced {} bytes of Markdown",
            convert_duration,
            markdown.len()
        );

        // Write Markdown content
        debug!("Writing Markdown content to: {}", md_file.display());
        let write_start = Instant::now();
        write(md_file, markdown).context("Failed to write Markdown file")?;
        let write_duration = write_start.elapsed();
        info!("Wrote Markdown content in {:.2?}", write_duration);

        let total_duration = start_time.elapsed();
        info!("Total conversion time for file: {:.2?}", total_duration);

        Ok(())
    }

    /// Convert HTML to Markdown using htmd
    fn convert_with_htmd(&self, html: &str) -> Result<String> {
        #[cfg(feature = "htmd")]
        {
            debug!("Converting HTML to Markdown with htmd feature");
            let md = htmd::HtmlToMarkdown::new()
                .convert(html)
                .map_err(|e| anyhow::anyhow!("htmd conversion failed: {}", e))?;
            Ok(md)
        }

        #[cfg(not(feature = "htmd"))]
        {
            Err(anyhow::anyhow!("htmd feature is not enabled"))
        }
    }

    /// Convert HTML to Markdown using fast_html2md
    fn convert_with_fast_html2md(&self, html: &str) -> Result<String> {
        #[cfg(feature = "fast-html2md")]
        {
            debug!("Converting HTML to Markdown with fast-html2md feature");
            // fast_html2md might panic on certain inputs, so catch potential panics
            let result = std::panic::catch_unwind(|| fast_html2md::convert_html(html));
            match result {
                Ok(Ok(md)) => Ok(md),
                Ok(Err(e)) => Err(anyhow::anyhow!("fast_html2md conversion failed: {}", e)),
                Err(_) => Err(anyhow::anyhow!("fast_html2md conversion panicked")),
            }
        }

        #[cfg(not(feature = "fast-html2md"))]
        {
            Err(anyhow::anyhow!("fast-html2md feature is not enabled"))
        }
    }

    /// A simple HTML to Markdown converter implementation (Fallback)
    /// This method implements basic conversions if features are disabled or conversion fails.
    #[cfg(not(any(feature = "htmd", feature = "fast-html2md")))]
    fn simple_html_to_markdown(&self, html: &str) -> String {
        warn!("Using simple fallback HTML to Markdown converter.");
        // Basic implementation focusing on text extraction and minimal formatting
        let mut output = String::new();
        let body_content = if let Some(body_start) = html.to_lowercase().find("<body") {
            if let Some(body_content_start) = html[body_start..].find('>') {
                let real_body_start = body_start + body_content_start + 1;
                if let Some(body_end) = html[real_body_start..].to_lowercase().find("</body>") {
                    &html[real_body_start..real_body_start + body_end]
                } else {
                    html // Fallback to full HTML if body end not found
                }
            } else {
                html
            }
        } else {
            html // Fallback if no body tag found
        };

        // Very basic tag stripping and text extraction
        let tag_regex = Regex::new(r"<[^>]+>").unwrap();
        let mut result = tag_regex.replace_all(body_content, " ").to_string();

        // Handle common entities
        result = result
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&nbsp;", " ");

        // Clean up whitespace
        let whitespace_regex = Regex::new(r"\s+").unwrap();
        result = whitespace_regex.replace_all(&result, " ").to_string();

        // Attempt to add some structure based on line breaks
        let lines: Vec<String> = result
            .split('\n')
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        output = lines.join("\n\n"); // Add double newline between potential paragraphs

        output.trim().to_string()
    }

    // Keep the simple converter stub if features *are* enabled, for fallback use
    #[cfg(any(feature = "htmd", feature = "fast-html2md"))]
    fn simple_html_to_markdown(&self, html: &str) -> String {
        warn!("Using simple fallback HTML to Markdown converter due to primary method failure.");
        // Basic implementation focusing on text extraction and minimal formatting
        let mut output = String::new();
        let body_content = if let Some(body_start) = html.to_lowercase().find("<body") {
            if let Some(body_content_start) = html[body_start..].find('>') {
                let real_body_start = body_start + body_content_start + 1;
                if let Some(body_end) = html[real_body_start..].to_lowercase().find("</body>") {
                    &html[real_body_start..real_body_start + body_end]
                } else {
                    html // Fallback to full HTML if body end not found
                }
            } else {
                html
            }
        } else {
            html // Fallback if no body tag found
        };

        // Very basic tag stripping and text extraction
        // We need regex even here if we want basic tag stripping fallback
        let tag_regex = regex::Regex::new(r"<[^>]+>").expect("Failed to compile tag regex");
        let mut result = tag_regex.replace_all(body_content, " ").to_string();

        // Handle common entities
        result = result
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&nbsp;", " ");

        // Clean up whitespace
        let whitespace_regex =
            regex::Regex::new(r"\s+").expect("Failed to compile whitespace regex");
        result = whitespace_regex.replace_all(&result, " ").to_string();

        // Attempt to add some structure based on line breaks
        let lines: Vec<String> = result
            .split('\n')
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        output = lines.join("\n\n"); // Add double newline between potential paragraphs

        output.trim().to_string()
    }

    /// Extract title from HTML content
    fn extract_title_from_html(&self, html: &str) -> Option<String> {
        if let Some(title_start) = html.to_lowercase().find("<title>") {
            if let Some(title_end) = html.to_lowercase()[title_start..].find("</title>") {
                let title_content = &html[title_start + 7..title_start + title_end];
                // Decode HTML entities in title
                return Some(html_escape::decode_html_entities(title_content.trim()).to_string());
            }
        }
        None
    }

    /// Convert HTML to Markdown using Jina Reader
    async fn convert_with_jina_reader(&self, html_file: &Path) -> Result<String> {
        let start_time = Instant::now();
        info!(
            "Starting Jina Reader conversion for file: {}",
            html_file.display()
        );

        // Extract original URL from filename (assuming crawler saved it like this)
        info!("Extracting original URL from filename");
        let filename = html_file.file_stem().unwrap().to_string_lossy();
        debug!("File stem: {}", filename);

        // Attempt to reconstruct the original URL from the filename format `host_path_parts`
        let url_string = filename.replace('_', "/").replace("-slash-", "/"); // Basic reconstruction attempt
        let original_url = match url::Url::parse(&format!("https://{}", url_string)) {
            Ok(url) => url.to_string(),
            Err(_) => {
                warn!(
                    "Could not reliably reconstruct URL from filename '{}', using raw HTML content for Jina (might fail)",
                    filename
                );
                // Fallback: Send raw HTML content? Jina might not support this well.
                // Or return error? Let's try returning an error as it's unlikely to work.
                return Err(anyhow::anyhow!(
                    "Cannot reconstruct URL from filename: {}",
                    filename
                ));
            }
        };

        info!("Reconstructed original URL: {}", original_url);

        // Prefix with Jina Reader URL
        let jina_url = format!("https://r.jina.ai/{}", original_url);
        info!("Jina Reader URL: {}", jina_url);

        // Download content from Jina Reader
        info!("Sending HTTP request to Jina Reader...");
        let req_start_time = Instant::now();

        let request = self.client.get(&jina_url);
        debug!("Request initialized, sending...");

        info!("Waiting for response from Jina Reader (timeout: 60s)...");
        let response_result = request.send().await;

        match response_result {
            Ok(response) => {
                let status = response.status();
                let req_duration = req_start_time.elapsed();
                info!(
                    "Received response from Jina Reader in {:.2?} with status: {}",
                    req_duration, status
                );

                if !status.is_success() {
                    error!("Jina Reader returned error status: {}", status);
                    // Read body even on error for potential details
                    let error_body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Failed to read error body".to_string());
                    error!("Jina Reader error body: {}", error_body);
                    return Err(anyhow::anyhow!(
                        "Jina Reader error: HTTP {} - {}",
                        status,
                        error_body
                    ));
                }

                info!("Reading response body...");
                let body_start_time = Instant::now();
                let body_result = response.text().await;

                match body_result {
                    Ok(body) => {
                        let body_duration = body_start_time.elapsed();
                        info!(
                            "Read response body in {:.2?} ({} bytes)",
                            body_duration,
                            body.len()
                        );
                        let total_duration = start_time.elapsed();
                        info!("Total Jina Reader processing time: {:.2?}", total_duration);
                        Ok(body)
                    }
                    Err(e) => {
                        let body_duration = body_start_time.elapsed();
                        error!(
                            "Failed to read response body after {:.2?}: {}",
                            body_duration, e
                        );
                        Err(anyhow::anyhow!(
                            "Failed to read Jina Reader response: {}",
                            e
                        ))
                    }
                }
            }
            Err(e) => {
                let req_duration = req_start_time.elapsed();
                error!(
                    "Failed to get response from Jina Reader after {:.2?}: {}",
                    req_duration, e
                );
                let error_message = if e.is_timeout() {
                    format!(
                        "Jina Reader request timed out after {:.2?}: {}",
                        req_duration, e
                    )
                } else if e.is_connect() {
                    format!("Failed to connect to Jina Reader: {}", e)
                } else {
                    format!("Jina Reader request failed: {}", e)
                };
                Err(anyhow::anyhow!(error_message))
            }
        }
    }
}
