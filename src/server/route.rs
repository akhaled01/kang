use crate::http::files::FileServer;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::{
    cgi::php::PhpExecContext,
    config::{RouteConfig, Config},
    http::{status::StatusCode, Request, Response},
    http::methods::Method,
    http::upload::UploadHandler
};

pub struct Route {
    pub path: String,
    pub root: Option<String>,
    pub index: Option<String>,
    pub methods: Vec<String>,
    pub directory_listing: bool,
    pub redirect: Option<Redirect>,
    pub cgi: Option<HashMap<String, String>>,
    pub client_max_body_size: Option<String>,
    pub config: Config,
}

pub struct Redirect {
    pub url: String,
    pub code: u16,
}

impl Route {
    pub fn handle(&self, request: Request) -> Response {
        // Check if method is allowed
        if !self
            .methods
            .contains(&request.method().as_str().to_string())
        {
            return Response::new(StatusCode::MethodNotAllowed);
        }

        if let Some(_) = &self.redirect {
            return self.handle_redirect();
        } else if let Some(_) = &self.cgi {
            return self.handle_cgi(request);
        } else {
            return self.handle_static(request);
        }
    }

    fn handle_redirect(&self) -> Response {
        let mut response =
            Response::new(StatusCode::from_u16(self.redirect.as_ref().unwrap().code).unwrap());
        response.set_header("Location", &self.redirect.as_ref().unwrap().url);
        response
    }

    fn handle_cgi(&self, request: Request) -> Response {
        let cgi_config = match &self.cgi {
            Some(config) => config,
            None => return Response::new(StatusCode::InternalServerError),
        };

        // Check if path ends with .php
        if !self.path.ends_with(".php") {
            return Response::new(StatusCode::NotImplemented);
        }

        // Get PHP handler
        let php_handler = match cgi_config.get(".php") {
            Some(handler) => handler,
            None => return Response::new(StatusCode::NotImplemented),
        };

        // Get script path
        let script_path = match &self.root {
            Some(root) => format!("{}{}", root, self.path),
            None => return Response::new(StatusCode::InternalServerError),
        };

        // Check if script exists
        if !Path::new(&script_path).exists() {
            return Response::new(StatusCode::NotFound);
        }

        // Create PHP execution context
        let mut php_ctx = PhpExecContext::new(php_handler.to_string(), script_path);

        php_ctx.add_env("REQUEST_METHOD", request.method().as_str());

        // Execute PHP script
        match php_ctx.exec() {
            Ok(output) => {
                let mut response = Response::new(StatusCode::Ok);
                response.set_header("Content-Type", "text/html");
                response.set_body(output.as_bytes().to_vec());
                response
            }
            Err(_) => Response::new(StatusCode::InternalServerError),
        }
    }

    fn handle_static(&self, request: Request) -> Response {
        // Handle file upload for POST requests
        if request.method() == &Method::POST {
            if !request.has_file_upload() {
                return Response::new(StatusCode::BadRequest);
            }

            let base_path = match &self.root {
                Some(root) => root,
                None => return Response::new(StatusCode::InternalServerError),
            };

            // Parse multipart form data
            let multipart_data = match request.parse_multipart_form_data() {
                Ok(data) => data,
                Err(_) => return Response::new(StatusCode::BadRequest),
            };

            // Create upload handler with client_max_body_size if specified
            let max_size = self.client_max_body_size.as_deref().unwrap_or("10M");
            let upload_handler = UploadHandler::new(max_size, base_path);

            // Handle the upload
            match upload_handler.handle_upload(&multipart_data) {
                Ok(files) => {
                    let mut response = Response::new(StatusCode::Ok);
                    let body = format!("Successfully uploaded {} files", files.len());
                    response.set_body(body.into_bytes());
                    response
                }
                Err(_) => Response::new(StatusCode::InternalServerError),
            }
        } else {
            // Handle GET requests - serve static files
            let base_path = match &self.root {
                Some(root) => root,
                None => return Response::new(StatusCode::InternalServerError),
            };

            // Get the relative path by removing the route path prefix
            let relative_path = request
                .path()
                .strip_prefix(&self.path)
                .unwrap_or(request.path());

            // Construct full path by joining base_path with the relative path
            let path = PathBuf::from(base_path).join(relative_path.trim_start_matches('/'));

            // Check if path exists
            if !path.exists() {
                return Response::new(StatusCode::NotFound);
            }

            // Handle directory
            if path.is_dir() {
                // Try to serve index file if specified
                if let Some(index) = &self.index {
                    let index_path = path.join(index);
                    if index_path.exists() {
                        return FileServer::serve_file(index_path);
                    }
                }

                // Show directory listing if enabled
                if self.directory_listing {
                    return FileServer::serve_directory_listing(&path, &self.path, &self.config);
                }

                return Response::new(StatusCode::NotFound);
            }

            // Serve the file
            FileServer::serve_file(path)
        }
    }
}

impl From<(RouteConfig, Config)> for Route {
    fn from((route_config, config): (RouteConfig, Config)) -> Self {
        Route {
            path: route_config.path,
            root: route_config.root,
            index: route_config.index,
            methods: route_config.methods,
            directory_listing: route_config.directory_listing,
            redirect: route_config.redirect.map(|r| Redirect {
                url: r.url,
                code: r.code,
            }),
            cgi: route_config.cgi,
            client_max_body_size: route_config.client_max_body_size,
            config,
        }
    }
}
