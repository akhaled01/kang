use super::route::Route;
use crate::config::{Config, ServerConfig};
use crate::http::{Request, Response, StatusCode};
use crate::info;

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
    fn handle_error(&self, status: StatusCode) -> Response {
        let mut res = Response::new(status);
        res.set_body_string(
            self.config
                .error_pages
                .pages
                .get(&status.to_string())
                .unwrap_or(&status.to_text()),
        );
        res
    }

    /// Adds a route to the Mux.
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    /// Validates the request by checking if the request matches a route and if the method is allowed.
    /// Returns the route if the request is valid, otherwise returns a status code.
    fn validate_request(&self, request: &Request) -> Result<Route, StatusCode> {
        info!("Validating request: {} {}", request.method(), request.path());
        let mut path_matched = false;
        
        for route in &self.routes {
            if request.path().starts_with(&route.path) {
                path_matched = true;
                if route.methods.contains(&request.method().as_str().to_string()) {
                    info!("Request matched route: {} {}", request.method(), route.path);
                    return Ok(route.clone());
                }
            }
        }
        
        // If we found a matching path but no matching method, return MethodNotAllowed
        // Otherwise return NotFound
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
