use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

use super::{errors::ConfigError, validator::ConfigValidator};
use crate::server::Server;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct SessionConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_minutes: i64,
    #[serde(default = "default_cookie_path")]
    pub cookie_path: String,
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
    #[serde(default = "default_cookie_http_only")]
    pub cookie_http_only: bool,
}

fn default_enabled() -> bool { false }
fn default_timeout() -> i64 { 60 }
fn default_cookie_path() -> String { "/".to_string() }
fn default_cookie_secure() -> bool { false }
fn default_cookie_http_only() -> bool { true }

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
    #[serde(default)]
    pub sessions: SessionConfig,
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
    #[serde(default)]
    pub sessions: SessionConfig,
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
    #[serde(default)]
    pub sessions_required: bool,
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

        if let Err(e) = ConfigValidator::validate(&config) {
            return Err(ConfigError::ValidationError(e));
        }

        Ok(config)
    }

    pub fn create_servers(&self) -> Vec<Server> {
        self.servers
            .iter()
            .map(|server_config| Server::new(server_config.clone(), self.clone()))
            .collect()
    }
}
