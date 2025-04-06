use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;

use crate::server::Server;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorPages {
    pub root: Option<String>,
    #[serde(rename = "404")]
    pub not_found: Option<String>,
    #[serde(rename = "500")]
    pub server_error: Option<String>,
    #[serde(rename = "403")]
    pub forbidden: Option<String>,
    #[serde(rename = "401")]
    pub unauthorized: Option<String>,
    #[serde(rename = "400")]
    pub bad_request: Option<String>,
    #[serde(rename = "406")]
    pub not_acceptable: Option<String>,
    #[serde(rename = "413")]
    pub request_entity_too_large: Option<String>,
    #[serde(rename = "415")]
    pub unsupported_media_type: Option<String>,
    #[serde(rename = "503")]
    pub service_unavailable: Option<String>,
    #[serde(rename = "501")]
    pub not_implemented: Option<String>,
    #[serde(rename = "502")]
    pub bad_gateway: Option<String>,
    #[serde(rename = "504")]
    pub gateway_timeout: Option<String>,
    #[serde(rename = "505")]
    pub http_version_not_supported: Option<String>,
    #[serde(rename = "507")]
    pub insufficient_storage: Option<String>,
    #[serde(rename = "509")]
    pub bandwidth_limit_exceeded: Option<String>,
    #[serde(rename = "510")]
    pub not_extended: Option<String>,
    #[serde(rename = "511")]
    pub network_authentication_required: Option<String>,
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
