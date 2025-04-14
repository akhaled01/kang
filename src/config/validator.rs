use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

use super::Config;

use crate::{error, warn};

/// Valid HTTP methods according to RFC 7231 and common extensions
const VALID_HTTP_METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "TRACE", "CONNECT",
];

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("No ports specified")]
    NoPortsSpecified,
    #[error("Invalid port: {0}")]
    InvalidPort(#[from] std::num::ParseIntError),
    #[error("Invalid directory listing response format: {0}")]
    InvalidDirectoryListingResponseFormat(String),
    #[error("Port {0} is already in use")]
    UsedPort(u16),
    #[error("Invalid server name: {0}")]
    InvalidServerName(String),
    #[error("Invalid CGI path: {0} for {1}")]
    InvalidCGIPath(String, String),
    #[error("Invalid host: {0}")]
    InvalidHost(String),
    #[error("Error page not found: {0}")]
    ErrorPageNotFound(String),
    #[error("Invalid route methods: {0}")]
    InvalidRouteMethods(String),
    #[error("Invalid route path: {0}")]
    InvalidRoutePath(String),
    #[error("Invalid root: {0}")]
    InvalidRoot(String),
    #[error("Duplicate port {0} found across servers")]
    DuplicatePort(u16),
    #[error("Duplicate route path {0} found in server")]
    DuplicateRoute(String),
    #[error("Redirect code {0} is not a valid HTTP redirect status code")]
    InvalidRedirectCode(u16),
    #[error("Invalid client_max_body_size format: {0}")]
    InvalidBodySizeFormat(String),
}

pub struct ConfigValidator;

impl ConfigValidator {
    /// Validates a client_max_body_size string format
    /// Format should be a number followed by K, M, or G (case insensitive)
    fn validate_body_size(size: &str) -> Result<(), ValidatorError> {
        let size = size.trim().to_uppercase();
        if !size.ends_with('K') && !size.ends_with('M') && !size.ends_with('G') {
            return Err(ValidatorError::InvalidBodySizeFormat(size));
        }
        let num = &size[..size.len() - 1];
        if num.parse::<u64>().is_err() {
            return Err(ValidatorError::InvalidBodySizeFormat(size));
        }
        Ok(())
    }

    pub fn validate(config: &Config) -> Result<(), ValidatorError> {
        let mut used_ports = HashSet::new();
        let mut has_critical_error = false;

        // Validate global config
        if let Some(size) = &config.global.client_max_body_size {
            if let Err(e) = Self::validate_body_size(size) {
                warn!("Invalid global client_max_body_size: {}", e);
            }
        }

        // Validate each server configuration
        for server in &config.servers {
            // Validate host (critical)
            if server.host.is_empty() {
                error!("Server has empty host");
                has_critical_error = true;
                continue;
            }

            // Validate server names (warning)
            for name in &server.server_name {
                if name.is_empty() {
                    warn!("Empty server name in server {}", server.host);
                }
            }

            // Validate ports (critical for duplicates)
            if server.ports.is_empty() {
                error!("No ports specified for server {}", server.host);
                has_critical_error = true;
                continue;
            }

            for &port in &server.ports {
                if !used_ports.insert(port) {
                    error!("Duplicate port {} found across servers", port);
                    return Err(ValidatorError::DuplicatePort(port));
                }
            }

            // Validate server-level client_max_body_size (warning)
            if let Some(size) = &server.client_max_body_size {
                if let Err(e) = Self::validate_body_size(size) {
                    warn!("Invalid client_max_body_size '{}' in server {}: {}", size, server.host, e);
                }
            }

            let mut used_routes = HashSet::new();

            // Validate routes
            for route in &server.routes {
                // Validate path and duplicates (warning)
                if route.path.is_empty() || !route.path.starts_with('/') {
                    warn!("Invalid route path '{}' in server {}", route.path, server.host);
                    continue;
                }

                if !used_routes.insert(route.path.clone()) {
                    warn!("Duplicate route '{}' in server {}", route.path, server.host);
                    continue;
                }

                // Validate methods (warning)
                if route.methods.is_empty() {
                    warn!("No methods specified for route '{}' in server {}", route.path, server.host);
                }

                for method in &route.methods {
                    if !VALID_HTTP_METHODS.contains(&method.to_uppercase().as_str()) {
                        warn!("Invalid HTTP method '{}' in route '{}'", method, route.path);
                    }
                }

                // Validate root (warning)
                if let Some(root) = &route.root {
                    if !Self::validate_path(root) {
                        warn!("Invalid root path '{}' for route '{}'", root, route.path);
                    }
                }

                // Validate redirect (warning)
                if let Some(redirect) = &route.redirect {
                    if !(300..=308).contains(&redirect.code) {
                        warn!("Invalid redirect code {} for route '{}'", redirect.code, route.path);
                    }
                }

                // Validate route-level client_max_body_size (warning)
                if let Some(size) = &route.client_max_body_size {
                    if let Err(e) = Self::validate_body_size(size) {
                        warn!("Invalid client_max_body_size '{}' in route '{}': {}", size, route.path, e);
                    }
                }
            }
        }

        if has_critical_error {
            error!("Critical configuration errors found");
            return Err(ValidatorError::NoPortsSpecified); // Using this as a generic critical error
        }

        Ok(())
    }

    fn validate_path(path: &str) -> bool {
        if path.starts_with("./") {
            true
        } else {
            Path::new(path).is_absolute()
        }
    }
}
