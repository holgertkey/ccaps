use std::fs;
use std::path::PathBuf;
use std::env;
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "ccaps-config.json";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    pub country_codes: Vec<String>,
    pub version: String,
}

impl Config {
    pub fn new() -> Self {
        Config {
            country_codes: Vec::new(),
            version: "0.7.0".to_string(),
        }
    }

    pub fn with_country_codes(country_codes: Vec<String>) -> Self {
        Config {
            country_codes,
            version: "0.7.0".to_string(),
        }
    }
}

// Get the path to the configuration file in AppData
pub fn get_config_path() -> Result<PathBuf, String> {
    // Use LOCALAPPDATA for application-specific data
    let app_data = env::var("LOCALAPPDATA")
        .or_else(|_| env::var("APPDATA"))
        .map_err(|_| "Cannot determine AppData directory".to_string())?;

    let config_dir = PathBuf::from(app_data).join("CCaps");

    // Create the directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Cannot create config directory: {}", e))?;
    }

    Ok(config_dir.join(CONFIG_FILE_NAME))
}

// Load configuration from file
pub fn load_config() -> Config {
    match get_config_path() {
        Ok(config_path) => {
            match fs::read_to_string(&config_path) {
                Ok(content) => {
                    match serde_json::from_str::<Config>(&content) {
                        Ok(config) => {
                            // Validate that the config has the correct version or is compatible
                            if config.version == "0.7.0" || config.version == "0.6.0" || config.version.starts_with("0.") {
                                return config;
                            } else {
                                eprintln!("Warning: Config version mismatch, using defaults");
                            }
                        },
                        Err(e) => {
                            eprintln!("Warning: Could not parse config file ({}), using defaults", e);
                        }
                    }
                },
                Err(_) => {
                    // Config file doesn't exist, that's ok - we'll create one when needed
                }
            }
        },
        Err(e) => {
            eprintln!("Warning: Could not determine config path ({}), using defaults", e);
        }
    }
    
    // Return default config if loading failed
    Config::new()
}

// Save configuration to file
pub fn save_config(config: &Config) -> Result<(), String> {
    let config_path = get_config_path()
        .map_err(|e| format!("Cannot determine config path: {}", e))?;
    
    let json_content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Cannot serialize config: {}", e))?;
    
    fs::write(&config_path, json_content)
        .map_err(|e| format!("Cannot write config file: {}", e))?;
    
    Ok(())
}

// Delete configuration file
pub fn delete_config() -> Result<(), String> {
    match get_config_path() {
        Ok(config_path) => {
            if config_path.exists() {
                fs::remove_file(&config_path)
                    .map_err(|e| format!("Cannot delete config file: {}", e))?;
            }
            Ok(())
        },
        Err(e) => Err(format!("Cannot determine config path: {}", e)),
    }
}

// Get configuration status information
pub fn get_config_info() -> (bool, Option<String>) {
    match get_config_path() {
        Ok(config_path) => {
            let exists = config_path.exists();
            let path_str = config_path.to_string_lossy().to_string();
            (exists, Some(path_str))
        },
        Err(_) => (false, None),
    }
}