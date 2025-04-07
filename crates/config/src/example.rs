//! This file contains an example of how to use the config crate.
//! It is not part of the library API, but serves as documentation.

use serde::{Deserialize, Serialize};
use anyhow::Result;
use super::{Config, Configuration};

/// Example configuration for a hypothetical crate
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ExampleConfig {
    /// Some string setting
    pub name: String,
    /// Some numeric setting
    pub value: i32,
    /// Some boolean flag
    pub enabled: bool,
}

impl Configuration for ExampleConfig {
    fn crate_name() -> &'static str {
        "example-crate"
    }
}

/// Example usage of the config crate
pub fn example_usage() -> Result<()> {
    // Load configuration or create default if it doesn't exist
    let mut config = Config::<ExampleConfig>::load_or_default()?;
    
    // Access and modify configuration values
    println!("Current config: {:?}", config.data);
    config.data.name = "New Name".to_string();
    config.data.value = 42;
    config.data.enabled = true;
    
    // Change how long backups are kept (default is 30 days)
    config.cleanup_backups_after_days = 60;
    
    // Save configuration (creates a backup first)
    config.save()?;
    
    // Get the location of the configuration file
    let config_path = Config::<ExampleConfig>::get_location();
    println!("Config file located at: {}", config_path.display());
    
    Ok(())
}

/// This is how a crate would implement its configuration
pub mod example_crate {
    use super::*;
    
    /// Initialize or load configuration for this crate
    pub fn init_config() -> Result<Config<ExampleConfig>> {
        let config = Config::<ExampleConfig>::load_or_default()?;
        Ok(config)
    }
    
    /// Get a setting from the configuration
    pub fn get_name() -> Result<String> {
        let config = init_config()?;
        Ok(config.data.name.clone())
    }
    
    /// Update a setting in the configuration
    pub fn set_name(name: &str) -> Result<()> {
        let mut config = init_config()?;
        config.data.name = name.to_string();
        config.save()?;
        Ok(())
    }
}