//! # MI4ULINGS Config
//!
//! Configuration management for the MI4ULINGS workspace.
//!
//! This crate provides functionality for storing, loading, and managing
//! configuration files for all crates in the MI4ULINGS workspace.
//!
//! ## Features:
//! - Store configs as TOML files in `.config` directory at workspace root
//! - Automatic backups before overwriting files
//! - Cleanup of old backups after configurable period (default: 30 days)
//! - Generic configuration trait for easy implementation in other crates
//!
//! ## Example:
//! See the `example` module for a complete example of how to use this crate.

// Example module with usage demonstration
pub mod example;

use std::fs::{self, create_dir_all, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, info, warn};

/// Represents a configuration object that can be serialized and deserialized.
pub trait Configuration: Serialize + DeserializeOwned + Default {
    /// The name of the crate or component this configuration belongs to.
    fn crate_name() -> &'static str;
}

/// Main configuration management struct
pub struct Config<T: Configuration> {
    /// The configuration data
    pub data: T,
    /// Number of days to keep backup files before cleaning them up (default: 30)
    pub cleanup_backups_after_days: u32,
}

impl<T: Configuration> Config<T> {
    /// Creates a new Config instance with default settings
    pub fn new() -> Self {
        Self {
            data: T::default(),
            cleanup_backups_after_days: 30, // Default value
        }
    }

    /// Gets the location of the configuration file
    pub fn get_location() -> PathBuf {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        workspace_root.join(".config").join(format!("{}.toml", T::crate_name()))
    }

    /// Gets the location of the backup directory
    fn get_backup_location() -> PathBuf {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        workspace_root.join(".config").join(".backup")
    }

    /// Loads configuration from file
    pub fn load() -> Result<Self> {
        let path = Self::get_location();
        debug!("Loading configuration from {}", path.display());

        if !path.exists() {
            return Err(anyhow::anyhow!("Configuration file does not exist"));
        }

        let mut file = File::open(&path)
            .with_context(|| format!("Failed to open configuration file: {}", path.display()))?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read configuration file: {}", path.display()))?;
        
        let config_data: T = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML: {}", path.display()))?;
        
        Ok(Self {
            data: config_data,
            cleanup_backups_after_days: 30, // Default value
        })
    }

    /// Loads configuration or creates default if not exists
    pub fn load_or_default() -> Result<Self> {
        match Self::load() {
            Ok(config) => {
                debug!("Loaded existing configuration");
                Ok(config)
            }
            Err(_) => {
                debug!("Configuration not found, creating default");
                let config = Self::new();
                config.save()?;
                Ok(config)
            }
        }
    }

    /// Creates a backup of the configuration file if it exists
    fn backup_file(&self) -> Result<()> {
        let config_path = Self::get_location();
        
        // If the file doesn't exist, no need to back it up
        if !config_path.exists() {
            return Ok(());
        }
        
        let backup_dir = Self::get_backup_location();
        create_dir_all(&backup_dir)
            .with_context(|| format!("Failed to create backup directory: {}", backup_dir.display()))?;
        
        // Generate timestamp for backup filename
        let now: DateTime<Local> = Local::now();
        let timestamp = now.format("%Y%m%d_%H%M%S");
        
        let filename = config_path.file_name().unwrap().to_string_lossy();
        let backup_path = backup_dir.join(format!("{}_{}", filename, timestamp));
        
        // Copy the file to backup location
        fs::copy(&config_path, &backup_path)
            .with_context(|| format!("Failed to create backup: {}", backup_path.display()))?;
        
        info!("Created backup at {}", backup_path.display());
        
        // Try to clean up old backups
        if let Err(e) = self.cleanup_old_backups() {
            warn!("Failed to clean up old backups: {}", e);
        }
        
        Ok(())
    }

    /// Saves configuration to file
    pub fn save(&self) -> Result<()> {
        // Create backup before overwriting
        self.backup_file()?;
        
        let path = Self::get_location();
        debug!("Saving configuration to {}", path.display());
        
        // Ensure the directory exists
        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        
        // Serialize to TOML
        let contents = toml::to_string(&self.data)
            .context("Failed to serialize configuration to TOML")?;
        
        // Write to file
        let mut file = File::create(&path)
            .with_context(|| format!("Failed to create configuration file: {}", path.display()))?;
        
        file.write_all(contents.as_bytes())
            .with_context(|| format!("Failed to write to configuration file: {}", path.display()))?;
        
        info!("Configuration saved to {}", path.display());
        Ok(())
    }

    /// Cleans up backup files older than cleanup_backups_after_days
    fn cleanup_old_backups(&self) -> Result<()> {
        let backup_dir = Self::get_backup_location();
        if !backup_dir.exists() {
            return Ok(());
        }
        
        let max_age = Duration::from_secs(self.cleanup_backups_after_days as u64 * 24 * 60 * 60);
        let now = SystemTime::now();
        
        let entries = fs::read_dir(&backup_dir)
            .with_context(|| format!("Failed to read backup directory: {}", backup_dir.display()))?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.file_name().unwrap().to_string_lossy().contains(T::crate_name()) {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age > max_age {
                                debug!("Removing old backup: {}", path.display());
                                if let Err(e) = fs::remove_file(&path) {
                                    warn!("Failed to remove old backup {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    
    #[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
    struct TestConfig {
        value: String,
    }
    
    impl Configuration for TestConfig {
        fn crate_name() -> &'static str {
            "test-config"
        }
    }
    
    #[test]
    fn test_get_location() {
        let path = Config::<TestConfig>::get_location();
        assert!(path.ends_with(".config/test-config.toml"));
    }
    
    // Additional tests would validate the save/load functionality
    // These would typically require a test directory to avoid interfering with real configs
}