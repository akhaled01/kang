use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;

use crate::server::server::Server;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub global: GlobalConfig,
    pub servers: Vec<ServerConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalConfig {
    pub client_max_body_size: Option<String>,
    pub response_format: Option<String>,
    pub cgi: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ErrorPages {
    #[serde(flatten)]
    pub pages: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub server_name: Vec<String>,
    pub error_pages: ErrorPages,
    pub host: String,
    pub ports: Vec<u16>,
    #[serde(default)]
    pub is_default: bool,
    pub routes: Vec<RouteConfig>,
    pub client_max_body_size: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteConfig {
    pub path: String,
    pub root: Option<String>,
    pub index: Option<String>,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub directory_listing: bool,
    pub redirect: Option<RedirectConfig>,
    pub cgi: Option<HashMap<String, String>>,
    pub client_max_body_size: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedirectConfig {
    pub url: String,
    pub code: u16,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn create_servers(&self) -> Vec<Server> {
        self.servers
            .iter()
            .map(|server_config| Server::new(server_config.clone(), self.clone()))
            .collect()
    }
}
