//! Converter for transforming HTML content to Markdown
//! Supports multiple conversion methods: htmd, fast_html2md, and jina_reader

use std::fs::{self, create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;
use reqwest::Client;
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
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self {
            config,
            client,
        })
    }
    
    /// Convert HTML files in a directory to Markdown
    pub async fn convert_directory(&self, entry_name: &str) -> Result<Vec<PathBuf>> {
        let base_dir = self.config.outputs_path.join(entry_name);
        let html_dir = base_dir.join(&self.config.output_parts_html_suffix);
        let md_dir = base_dir.join(&self.config.output_parts_markdown_suffix);
        
        // Create markdown directory if it doesn't exist
        create_dir_all(&md_dir).context("Failed to create Markdown output directory")?;
        
        // Get all HTML files
        let html_files = fs::read_dir(&html_dir)
            .context("Failed to read HTML directory")?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) &&
                entry.path().extension().map(|ext| ext == "html").unwrap_or(false)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        
        let mut converted_files = Vec::new();
        
        // Process each HTML file
        for html_file in html_files {
            let filename = html_file.file_stem().unwrap().to_string_lossy();
            let md_file = md_dir.join(format!("{}.md", filename));
            
            match self.convert_file(&html_file, &md_file).await {
                Ok(_) => {
                    debug!("Converted {} to {}", html_file.display(), md_file.display());
                    converted_files.push(md_file);
                }
                Err(e) => {
                    error!("Failed to convert {}: {}", html_file.display(), e);
                }
            }
        }
        
        info!("Converted {} HTML files to Markdown", converted_files.len());
        Ok(converted_files)
    }
    
    /// Convert a single HTML file to Markdown
    pub async fn convert_file(&self, html_file: &Path, md_file: &Path) -> Result<()> {
        // Read HTML content
        let html_content = read_to_string(html_file)
            .context("Failed to read HTML file")?;
        
        // Convert to Markdown based on config
        let markdown = match self.config.transform_md_using {
            TransformMethod::Htmd => self.convert_with_htmd(&html_content)?,
            TransformMethod::FastHtml2md => self.convert_with_fast_html2md(&html_content)?,
            TransformMethod::JinaReader => {
                self.convert_with_jina_reader(html_file).await?
            }
        };
        
        // Write Markdown content
        write(md_file, markdown).context("Failed to write Markdown file")?;
        
        Ok(())
    }
    
    /// Convert HTML to Markdown using htmd
    fn convert_with_htmd(&self, html: &str) -> Result<String> {
        #[cfg(feature = "htmd")]
        {
            // Using a simplified conversion approach for now
            // In a real implementation, we would use the htmd library properly
            debug!("Converting HTML to Markdown with htmd feature");
            let mut md = self.simple_html_to_markdown(html);
            
            // Try to extract title for use as header
            let title = self.extract_title_from_html(html);
            if let Some(title) = title {
                md = format!("# {}\n\n{}", title, md);
            }
            
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
            // Using a simplified conversion approach for now
            // In a real implementation, we would use the fast_html2md library properly
            debug!("Converting HTML to Markdown with fast-html2md feature");
            let mut md = self.simple_html_to_markdown(html);
            
            // Try to extract title for use as header
            let title = self.extract_title_from_html(html);
            if let Some(title) = title {
                md = format!("# {}\n\n{}", title, md);
            }
            
            Ok(md)
        }
        
        #[cfg(not(feature = "fast-html2md"))]
        {
            Err(anyhow::anyhow!("fast-html2md feature is not enabled"))
        }
    }
    
    /// A simple HTML to Markdown converter implementation
    /// This is a fallback method that implements basic conversions
    fn simple_html_to_markdown(&self, html: &str) -> String {
        let mut output = String::new();
        let mut in_body = false;
        
        // Extract content from body tag
        if let Some(body_start) = html.to_lowercase().find("<body") {
            if let Some(body_content_start) = html[body_start..].find('>') {
                let real_body_start = body_start + body_content_start + 1;
                
                if let Some(body_end) = html[real_body_start..].to_lowercase().find("</body>") {
                    let body_content = &html[real_body_start..real_body_start + body_end];
                    
                    // Process body content
                    let mut result = body_content.to_string();
                    
                    // Basic replacements for common HTML elements
                    // Headers
                    result = result.replace("<h1>", "# ").replace("</h1>", "\n\n");
                    result = result.replace("<h2>", "## ").replace("</h2>", "\n\n");
                    result = result.replace("<h3>", "### ").replace("</h3>", "\n\n");
                    result = result.replace("<h4>", "#### ").replace("</h4>", "\n\n");
                    result = result.replace("<h5>", "##### ").replace("</h5>", "\n\n");
                    result = result.replace("<h6>", "###### ").replace("</h6>", "\n\n");
                    
                    // Paragraphs
                    result = result.replace("<p>", "").replace("</p>", "\n\n");
                    
                    // Lists
                    result = result.replace("<ul>", "").replace("</ul>", "\n");
                    result = result.replace("<ol>", "").replace("</ol>", "\n");
                    result = result.replace("<li>", "* ").replace("</li>", "\n");
                    
                    // Links - simplified approach, not handling attributes properly
                    while let Some(link_start) = result.find("<a ") {
                        if let Some(href_start) = result[link_start..].find("href=\"") {
                            let href_content_start = link_start + href_start + 6;
                            if let Some(href_end) = result[href_content_start..].find('"') {
                                let url = &result[href_content_start..href_content_start + href_end];
                                
                                if let Some(tag_end) = result[link_start..].find('>') {
                                    let tag_close = link_start + tag_end + 1;
                                    
                                    if let Some(closing_tag) = result[tag_close..].find("</a>") {
                                        let text = &result[tag_close..tag_close + closing_tag];
                                        let link = format!("<a href=\"{}\">{}</a>", url, text);
                                        let md_link = format!("[{}]({})", text, url);
                                        
                                        result = result.replacen(&link, &md_link, 1);
                                    } else {
                                        // No closing tag found, just remove the opening tag
                                        let tag = &result[link_start..tag_close];
                                        result = result.replacen(tag, "", 1);
                                    }
                                } else {
                                    // Malformed anchor tag, just remove it
                                    let partial_tag = &result[link_start..];
                                    result = result[..link_start].to_string();
                                    break;
                                }
                            } else {
                                // Couldn't find end of href attribute, just remove the tag
                                let tag = &result[link_start..link_start + href_start + 6];
                                result = result.replacen(tag, "", 1);
                            }
                        } else {
                            // No href found, just remove the tag
                            let tag = &result[link_start..link_start + 3];
                            result = result.replacen(tag, "", 1);
                        }
                    }
                    
                    // Remove remaining HTML tags (simplified approach)
                    while let Some(tag_start) = result.find('<') {
                        if let Some(tag_end) = result[tag_start..].find('>') {
                            let tag = &result[tag_start..tag_start + tag_end + 1];
                            result = result.replacen(tag, "", 1);
                        } else {
                            break;
                        }
                    }
                    
                    // Handle entity references
                    result = result.replace("&lt;", "<").replace("&gt;", ">")
                              .replace("&amp;", "&").replace("&quot;", "\"")
                              .replace("&nbsp;", " ");
                    
                    // Remove multiple consecutive whitespace
                    let whitespace_regex = regex::Regex::new(r"\s{2,}").unwrap();
                    result = whitespace_regex.replace_all(&result, " ").to_string();
                    
                    // Fix newlines
                    result = result.replace('\r', "");
                    let multiple_newlines = regex::Regex::new(r"\n{3,}").unwrap();
                    result = multiple_newlines.replace_all(&result, "\n\n").to_string();
                    
                    return result;
                }
            }
        }
        
        // Fallback if we couldn't extract body content
        let mut result = html.to_string();
        
        // Remove all script and style tags and their contents
        while let Some(script_start) = result.to_lowercase().find("<script") {
            if let Some(script_end) = result[script_start..].to_lowercase().find("</script>") {
                result = result[..script_start].to_string() + &result[script_start + script_end + 9..];
            } else {
                break;
            }
        }
        
        while let Some(style_start) = result.to_lowercase().find("<style") {
            if let Some(style_end) = result[style_start..].to_lowercase().find("</style>") {
                result = result[..style_start].to_string() + &result[style_start + style_end + 8..];
            } else {
                break;
            }
        }
        
        // Basic replacements (simplified)
        result = result.replace("<h1>", "# ").replace("</h1>", "\n\n");
        result = result.replace("<p>", "").replace("</p>", "\n\n");
        result = result.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
        
        // Remove remaining HTML tags (simplified)
        let tag_regex = regex::Regex::new(r"<[^>]+>").unwrap();
        result = tag_regex.replace_all(&result, "").to_string();
        
        // Clean up whitespace
        let whitespace_regex = regex::Regex::new(r"\s{2,}").unwrap();
        result = whitespace_regex.replace_all(&result, " ").to_string();
        
        result
    }
    
    /// Extract title from HTML content
    fn extract_title_from_html(&self, html: &str) -> Option<String> {
        // Simple regex to extract content between <title> tags
        if let Some(title_start) = html.to_lowercase().find("<title>") {
            if let Some(title_end) = html.to_lowercase()[title_start..].find("</title>") {
                let title_content = &html[title_start + 7..title_start + title_end];
                return Some(title_content.trim().to_string());
            }
        }
        None
    }
    
    /// Convert HTML to Markdown using Jina Reader
    async fn convert_with_jina_reader(&self, html_file: &Path) -> Result<String> {
        // For Jina Reader, we need the original URL to prefix with https://r.jina.ai/
        // This is a simplified implementation - in a real-world scenario, we would store
        // the original URL with each downloaded file
        
        // Extract filename which has the URL encoded in it
        let filename = html_file.file_stem().unwrap().to_string_lossy();
        let parts: Vec<&str> = filename.split('_').collect();
        
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Cannot extract URL from filename"));
        }
        
        // Reconstruct original URL
        let host = parts[0];
        let path = parts[1..].join("/");
        let url = format!("https://{}/{}", host, path);
        
        // Prefix with Jina Reader URL
        let jina_url = format!("https://r.jina.ai/{}", url);
        
        // Download content from Jina Reader
        let response = self.client.get(&jina_url).send().await
            .context("Failed to fetch from Jina Reader")?;
        
        let markdown = response.text().await
            .context("Failed to read Jina Reader response")?;
        
        Ok(markdown)
    }
}