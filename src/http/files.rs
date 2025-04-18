use std::fs;
use std::path::PathBuf;
use serde_json::json;

use crate::http::status::StatusCode;
use crate::http::Response;
use crate::config::config::Config;

pub struct FileServer;

impl FileServer {
    pub fn serve_file(path: PathBuf) -> Response {
        match fs::read(&path) {
            Ok(content) => {
                let mut response = Response::new(StatusCode::Ok);

                // Set content type based on extension
                if let Some(ext) = path.extension() {
                    let content_type = match ext.to_str().unwrap_or("") {
                        "html" => "text/html",
                        "css" => "text/css",
                        "js" => "application/javascript",
                        "png" => "image/png",
                        "jpg" | "jpeg" => "image/jpeg",
                        "gif" => "image/gif",
                        "svg" => "image/svg+xml",
                        "webp" => "image/webp",
                        "ico" => "image/x-icon",
                        "pdf" => "application/pdf",
                        "json" => "application/json",
                        "xml" => "application/xml",
                        "txt" => "text/plain",
                        _ => "application/octet-stream",
                    };
                    response.set_header("Content-Type", content_type);
                }

                response.set_body(content);
                response
            }
            Err(_) => Response::new(StatusCode::InternalServerError),
        }
    }

    pub fn serve_directory_listing(path: &PathBuf, request_path: &str, config: &Config) -> Response {
        match fs::read_dir(path) {
            Ok(entries) => {
                let entries_vec: Vec<_> = entries
                    .filter_map(Result::ok)
                    .map(|entry| {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let link = format!("{}/{}", request_path.trim_end_matches('/'), name);
                        (name, link)
                    })
                    .collect();

                let mut response = Response::new(StatusCode::Ok);

                let temp =String::from("html"); //TODO: need to convert this to an enum omg
                
                let format = config.global.response_format.as_ref().unwrap_or(&temp);
                let format_str = format.as_str();
                match format_str {
                    "json" => {
                        let json_content = json!({
                            "directory": request_path,
                            "entries": entries_vec.iter().map(|(name, link)| {
                                json!({
                                    "name": name,
                                    "link": link
                                })
                            }).collect::<Vec<_>>()
                        });
                        response.set_header("Content-Type", "application/json");
                        response.set_body(json_content.to_string().into_bytes());
                    },
                    _ => {
                        let mut html = String::from("<html><body><h1>Directory listing</h1><ul>");
                        for (name, link) in entries_vec {
                            html.push_str(&format!("<li><a href=\"{}\">{}</a></li>", link, name));
                        }
                        html.push_str("</ul></body></html>");
                        response.set_header("Content-Type", "text/html");
                        response.set_body(html.into_bytes());
                    }
                };
                response
            }
            Err(_) => Response::new(StatusCode::InternalServerError),
        }
    }
}
