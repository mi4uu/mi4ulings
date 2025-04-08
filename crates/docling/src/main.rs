//! CLI application for mi4ulings-docling
//! 
//! Provides commands for managing web crawling tasks:
//! - add [url] - Add a new URL to crawl
//! - stop [name] - Disable a URL entry
//! - list - List all URL entries
//! - remove [name] - Remove a URL entry
//! - start [name] - Enable and process a URL entry

use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::fs::create_dir_all;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mi4ulings_config::Config;
use tokio::runtime::Runtime;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime}, 
    EnvFilter, 
    prelude::*
};

use mi4ulings_docling::{self, DoclingConfig};

/// Docling - Web crawler and document processor
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Command to execute
    #[clap(subcommand)]
    command: Commands,
}

/// Available commands
#[derive(Subcommand)]
enum Commands {
    /// Add a new URL to crawl
    Add {
        /// URL to crawl
        #[clap(required = true)]
        url: String,
        
        /// Name for this entry (defaults to domain name)
        #[clap(short, long)]
        name: Option<String>,
        
        /// Crawl depth (how many levels of links to follow)
        #[clap(short, long)]
        depth: Option<u32>,
    },
    
    /// Stop (disable) a URL entry
    Stop {
        /// Name of the entry to stop
        #[clap(required = true)]
        name: String,
    },
    
    /// List all URL entries
    List,
    
    /// Remove a URL entry
    Remove {
        /// Name of the entry to remove
        #[clap(required = true)]
        name: String,
    },
    
    /// Start (enable and process) a URL entry
    Start {
        /// Name of the entry to start
        #[clap(required = true)]
        name: String,
    },
}

/// Initialize logging system with both console and file output
fn init_logging() -> Result<()> {
    // Get configuration to access log paths
    let config = Config::<DoclingConfig>::load_or_default()?;
    let config_data = config.data;
    
    // Create logs directory
    let log_dir = config_data.logs_path.clone();
    create_dir_all(&log_dir)
        .context(format!("Failed to create log directory: {}", log_dir.display()))?;
    
    // Determine log level from environment or use default
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                EnvFilter::new("debug")
            } else {
                EnvFilter::new("info")
            }
        });
    
    // Create a log file with timestamp
    let now = chrono::Local::now();
    let log_file_name = format!("docling_{}.log", now.format("%Y%m%d_%H%M%S"));
    let log_file_path = log_dir.join(log_file_name);
    
    // Log info about the log file
    println!("Logs will be written to: {}", log_file_path.display());
    
    // Create file appender
    let file_appender = tracing_appender::rolling::daily(
        &log_dir, 
        "docling.log"
    );
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Also create a non-blocking stdout writer
    let (stdout_writer, _guard_stdout) = tracing_appender::non_blocking(std::io::stdout());
    
    // Configure and install the tracing subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        // Add console logger with minimal formatting
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(stdout_writer)
                .with_ansi(true)
                .with_span_events(FmtSpan::CLOSE)
        )
        // Add file logger with more detailed formatting
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false) // No ANSI colors in log file
                .with_timer(UtcTime::rfc_3339())
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
        )
        .try_init()
        .context("Failed to set up tracing subscriber")?;
    
    info!("Logging system initialized at level: {}", if cfg!(debug_assertions) { "DEBUG" } else { "INFO" });
    info!("Log file: {}", log_file_path.display());
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration first (needed for logging)
    let config = Config::<DoclingConfig>::load_or_default()?;
    
    // Initialize logging
    init_logging()?;
    
    // Parse command line arguments (only once)
    let cli = Cli::parse();
    
    // Log start of application
    info!("Starting mi4ulings-docling v{}", env!("CARGO_PKG_VERSION"));
    debug!("Using configuration: {:#?}", config.data);
    
    // Execute command
    match cli.command {
        Commands::Add { url, name, depth } => {
            let name_str = name.as_deref();
            mi4ulings_docling::add_url(&url, name_str, depth)?;
            println!("Added URL: {}", url);
        }
        
        Commands::Stop { name } => {
            mi4ulings_docling::stop_url(&name)?;
            println!("Stopped entry: {}", name);
        }
        
        Commands::List => {
            let entries = mi4ulings_docling::list_urls()?;
            
            // Load config to get retry count
            let config = Config::<DoclingConfig>::load_or_default()?;
            let retry_count = config.data.retry_count;
            
            if entries.is_empty() {
                println!("No entries found");
            } else {
                println!("URL entries:");
                println!("{:<20} {:<30} {:<8} {:<5} {:<10} {:<10} {:<10}", "NAME", "URL", "STATUS", "DEPTH", "TRIES", "RETRIES", "LAST DOWNLOAD");
                println!("{}", "-".repeat(110));
                
                for entry in entries {
                    let status = match entry.status {
                        mi4ulings_docling::CrawlStatus::Enabled => "Enabled",
                        mi4ulings_docling::CrawlStatus::Disabled => "Disabled",
                        mi4ulings_docling::CrawlStatus::Failed => "Failed",
                    };
                    
                    let last_download = entry.last_download
                        .map(|dt| dt.to_string())
                        .unwrap_or_else(|| "Never".to_string());
                    
                    // Calculate try count based on last_try and last_fail
                    let try_count = if entry.last_try.is_some() {
                        if entry.last_fail.is_some() {
                            // If both are set, count them as separate tries
                            "Multiple"
                        } else {
                            // If only last_try is set, it's at least one try
                            "1+"
                        }
                    } else {
                        "0"
                    };
                    
                    println!("{:<20} {:<30} {:<8} {:<5} {:<10} {:<10} {:<10}",
                             entry.name,
                             if entry.url.len() > 30 { 
                                 format!("{}...", &entry.url[..27]) 
                             } else { 
                                 entry.url.clone() 
                             },
                             status,
                             entry.crawl_depth,
                             try_count,
                             retry_count,
                             if last_download.len() > 10 { 
                                 format!("{}...", &last_download[..7]) 
                             } else { 
                                 last_download
                             });
                }
            }
        }
        
        Commands::Remove { name } => {
            mi4ulings_docling::remove_url(&name)?;
            println!("Removed entry: {}", name);
        }
        
        Commands::Start { name } => {
            // First enable the entry
            mi4ulings_docling::start_url(&name)?;
            println!("Starting entry: {}", name);
            
            // Then run the entry
            match mi4ulings_docling::run_entry(&name).await {
                Ok(result_file) => {
                    println!("Successfully processed entry: {}", name);
                    println!("Result file: {}", result_file.display());
                }
                Err(e) => {
                    error!("Failed to process entry: {}", e);
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            }
        }
    }
    
    Ok(())
}