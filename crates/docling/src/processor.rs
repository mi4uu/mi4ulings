//! Processor for cleaning and combining Markdown content
//! Handles removing images, media, excessive whitespace, and non-domain links

use std::fs::{self, create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};
use std::collections::HashSet;

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};
use url::Url;

use crate::DoclingConfig;

/// Processor for Markdown content
pub struct Processor {
    /// Configuration for the processor
    config: DoclingConfig,
}

impl Processor {
    /// Create a new processor with the given configuration
    pub fn new(config: DoclingConfig) -> Self {
        Self {
            config,
        }
    }
    
    /// Process Markdown files for an entry
    pub fn process_entry(&self, entry_name: &str, base_url: &str) -> Result<PathBuf> {
        let base_dir = self.config.outputs_path.join(entry_name);
        let md_dir = base_dir.join(&self.config.output_parts_markdown_suffix);
        let result_dir = base_dir.join(&self.config.output_parts_markdown_results_suffix);
        
        // Create result directory if it doesn't exist
        create_dir_all(&result_dir).context("Failed to create result directory")?;
        
        // Get all Markdown files
        let md_files = fs::read_dir(&md_dir)
            .context("Failed to read Markdown directory")?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) &&
                entry.path().extension().map(|ext| ext == "md").unwrap_or(false)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        
        // Combine and clean Markdown files
        let combined_content = self.combine_files(&md_files, base_url)?;
        let output_file = result_dir.join(format!("{}.md", entry_name));
        
        // Write result
        write(&output_file, combined_content).context("Failed to write result file")?;
        
        info!(
            "Created combined and cleaned Markdown file: {}",
            output_file.display()
        );
        
        Ok(output_file)
    }
    
    /// Combine multiple Markdown files into one
    fn combine_files(&self, files: &[PathBuf], base_url: &str) -> Result<String> {
        let mut combined = String::new();
        
        // Try to parse the base URL
        let parsed_base_url = Url::parse(base_url).context("Invalid base URL")?;
        let base_domain = parsed_base_url.host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host"))?
            .to_string();
        
        // Process each file
        for file in files {
            let content = read_to_string(file)
                .with_context(|| format!("Failed to read file: {}", file.display()))?;
            
            // Clean the content
            let cleaned = self.clean_content(&content, &base_domain);
            
            // Add section header based on filename
            let filename = file.file_stem().unwrap().to_string_lossy();
            combined.push_str(&format!("\n\n## {}\n\n", filename));
            combined.push_str(&cleaned);
        }
        
        // Final cleanup of the combined content
        self.final_cleanup(&combined)
    }
    
    /// Clean Markdown content by removing images, media, and non-domain links
    fn clean_content(&self, content: &str, base_domain: &str) -> String {
        let mut cleaned = String::new();
        
        // Process each line
        for line in content.lines() {
            // Skip image lines (Markdown format)
            if line.trim().starts_with("![") && line.contains("](") && line.contains(")") {
                continue;
            }
            
            // Process links to keep only domain links
            let processed_line = self.process_links(line, base_domain);
            
            // Add line to cleaned content
            cleaned.push_str(&processed_line);
            cleaned.push('\n');
        }
        
        cleaned
    }
    
    /// Process links in a line, keeping only those from the specified domain
    fn process_links(&self, line: &str, base_domain: &str) -> String {
        let mut result = line.to_string();
        let mut link_start = 0;
        
        // Look for Markdown links [text](url)
        while let Some(pos) = result[link_start..].find("](") {
            let real_pos = link_start + pos;
            let text_start = result[..real_pos].rfind('[');
            
            if let Some(text_start) = text_start {
                let url_start = real_pos + 2;
                let url_end = if let Some(end) = result[url_start..].find(')') {
                    url_start + end
                } else {
                    break;
                };
                
                let url = &result[url_start..url_end];
                
                // Check if URL is from the base domain
                if let Ok(parsed_url) = Url::parse(url) {
                    if let Some(host) = parsed_url.host_str() {
                        if !host.contains(base_domain) {
                            // Replace with just the text
                            let text = &result[text_start + 1..real_pos];
                            let link = format!("[{}]({})", text, url);
                            result = result.replace(&link, text);
                            // Reset position because the string changed
                            link_start = 0;
                            continue;
                        }
                    }
                }
            }
            
            link_start = real_pos + 2;
        }
        
        result
    }
    
    /// Perform final cleanup on the combined content
    fn final_cleanup(&self, content: &str) -> Result<String> {
        let mut result = content.to_string();
        
        // Remove multiple consecutive blank lines
        let mut prev_blank = false;
        let mut cleaned_lines = Vec::new();
        
        for line in result.lines() {
            let is_blank = line.trim().is_empty();
            
            if is_blank && prev_blank {
                continue;
            }
            
            cleaned_lines.push(line);
            prev_blank = is_blank;
        }
        
        result = cleaned_lines.join("\n");
        
        // Remove HTML image tags that might have been missed
        result = self.remove_html_tags(&result, "img");
        
        // Remove HTML video/audio tags
        result = self.remove_html_tags(&result, "video");
        result = self.remove_html_tags(&result, "audio");
        
        Ok(result)
    }
    
    /// Remove HTML tags of a specific type
    fn remove_html_tags(&self, content: &str, tag: &str) -> String {
        let mut result = content.to_string();
        
        // Find and remove opening and closing tags
        let open_tag = format!("<{}", tag);
        let close_tag = format!("</{}>", tag);
        
        while let Some(start) = result.find(&open_tag) {
            if let Some(end) = result[start..].find('>') {
                let real_end = start + end + 1;
                
                // Check for self-closing tag
                if result[start..real_end].ends_with("/>") {
                    result = result[..start].to_string() + &result[real_end..];
                    continue;
                }
                
                // Look for closing tag
                if let Some(close_start) = result[real_end..].find(&close_tag) {
                    let real_close_end = real_end + close_start + close_tag.len();
                    result = result[..start].to_string() + &result[real_close_end..];
                } else {
                    // No closing tag found, just remove the opening tag
                    result = result[..start].to_string() + &result[real_end..];
                }
            } else {
                break;
            }
        }
        
        result
    }
}