use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    IoError(#[from] std::io::Error),
    ParseError(#[from] serde_json::Error),
    ValidationError(#[from] crate::config::validator::ValidatorError),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
       match self {
           ConfigError::IoError(e) => write!(f, "IO Error: {}", e),
           ConfigError::ParseError(e) => write!(f, "Parse Error: {}", e),
           ConfigError::ValidationError(e) => write!(f, "Validation Error: {}", e),
       }
    }
}