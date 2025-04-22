use super::route::Route;
use crate::config::{Config, ServerConfig};
use crate::http::{Request, Response, StatusCode};
use crate::{debug, error, info};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
/// A mux is an HTTP multiplexer that routes incoming requests to the appropriate handler.
/// errors are handled in accordance with the config of the server that owns the Mux
pub struct Mux {
    pub routes: Vec<Route>,
    pub config: ServerConfig,
}

impl Mux {
    pub fn new(config: ServerConfig, global_cfg: Config) -> Self {
        Mux {
            routes: config
                .routes
                .clone()
                .into_iter()
                .map(|r| Route::from((r, global_cfg.clone())))
                .collect(),
            config,
        }
    }

    /// Handles an error by generating a response from the error pages config.
    /// If the status code is not found in the error pages config, it will fall
    /// back to the status code's text representation.
    fn serve_error_page(&self, error_path: &str) -> String {
        // Convert relative path to absolute path from project root
        let path = if error_path.starts_with("./") {
            PathBuf::from(error_path.strip_prefix("./").unwrap_or(error_path))
        } else {
            PathBuf::from(error_path)
        };

        if let Ok(content) = fs::read_to_string(&path) {
            content
        } else {
            error!("Failed to read error page: {}", path.display());
            "Error page not found".to_string()
        }
    }

    fn handle_error(&self, status: StatusCode) -> Response {
        let mut res = Response::new(status);
        let content = self.config
            .error_pages
            .pages
            .get(&status.to_string())
            .map(|path| self.serve_error_page(path))
            .unwrap_or_else(|| status.to_text());
            
        res.set_body_string(&content);
        res.set_header("Content-Type", "text/html");
        res
    }

    /// Adds a route to the Mux.
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    /// Validates the request by checking if the request matches a route and if the method is allowed.
    /// Returns the route if the request is valid, otherwise returns a status code.
    fn validate_request(&self, request: &Request) -> Result<Route, StatusCode> {
        info!(
            "Validating request: {} {}",
            request.method(),
            request.path()
        );
        let mut path_matched = false;
        let request_path = request.path().trim_end_matches('/');

        // Sort routes by path length in descending order to match most specific routes first
        let mut routes = self.routes.clone();
        routes.sort_by(|a, b| b.path.len().cmp(&a.path.len()));

        for route in &routes {
            // debug!("validating against: {:?}", route.path.clone());
            let route_path = route.path.trim_end_matches('/');
            
            // Special case for root path
            if route_path == "" && request_path == "" {
                path_matched = true;
                if route.methods.contains(&request.method().as_str().to_string()) {
                    info!("Request matched root route: {}", request.method());
                    return Ok(route.clone());
                }
                continue;
            }

            // For non-root paths, ensure exact match or proper path separation
            // Handle exact match case
            if request_path == route_path {
                path_matched = true;
                if route.methods.contains(&request.method().as_str().to_string()) {
                    info!("Request matched exact route: {} {}", request.method(), route.path);
                    return Ok(route.clone());
                }
                continue;
            }

            // Handle subpath matching
            let route_with_slash = if route_path.ends_with('/') {
                route_path.to_string()
            } else {
                format!("{}/", route_path)
            };

            if route_path != "" { // prevent root path from matching everything
                // For file serving routes (those with root_dir), allow nested paths
                let is_file_route = route.root.is_some();
                let path_matches = if is_file_route {
                    request_path.starts_with(&route_with_slash) || request_path.starts_with(route_path)
                } else {
                    // For non-file routes, require exact match or trailing slash only
                    request_path == route_path || request_path == route_with_slash
                };

                if path_matches {
                    path_matched = true;
                    if route.methods.contains(&request.method().as_str().to_string()) {
                        info!("Request matched route: {} {}", request.method(), route.path);
                        return Ok(route.clone());
                    }
                }
            }

            // Check if this route can handle file operations
            if let Some(root_dir) = &route.root {
                let file_path = PathBuf::from(root_dir).join(request_path.trim_start_matches('/'));
                debug!("Checking file existence: {:?}", file_path);
                if file_path.exists() && file_path.is_file() {
                    debug!("File exists: {:?}", file_path);
                    path_matched = true;
                    if route.methods.contains(&request.method().as_str().to_string()) {
                        info!("Request matched file operation: {} {} (route: {})", request.method(), request_path, route.path);
                        return Ok(route.clone());
                    }
                }
            }
        }

        if path_matched {
            Err(StatusCode::MethodNotAllowed)
        } else {
            Err(StatusCode::NotFound)
        }
    }

    /// Handles an incoming HTTP request by routing it to the appropriate handler.
    /// If the request matches a route, the route's handler is called.
    /// If the request does not match any route, a 404 Not Found response is returned.
    pub fn handle(&self, request: Request) -> Response {
        match self.validate_request(&request) {
            Ok(route) => match route.handle(request) {
                Ok(response) => response,
                Err(status) => self.handle_error(status),
            },
            Err(status) => self.handle_error(status),
        }
    }
}
