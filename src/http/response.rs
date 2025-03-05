use std::collections::HashMap;
use std::fmt::Write;

use crate::http::Headers;

#[derive(Debug)]
pub struct Response {
    status_code: u16,
    status_text: String,
    headers: Headers,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: u16, status_text: &str) -> Self {
        let mut response = Response {
            status_code,
            status_text: status_text.to_string(),
            headers: Headers::new(),
            body: Vec::new(),
        };

        // Add default headers
        response.headers.add("Server", "Kang");
        response.headers.add("Connection", "close");

        response
    }

    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.add(key, value);
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = body;
        self.set_header("Content-Length", &self.body.len().to_string());
    }

    pub fn set_body_string(&mut self, body: &str) {
        self.set_body(body.as_bytes().to_vec());
    }

    // Create a file upload success response
    pub fn file_upload_success(file_count: usize) -> Self {
        let mut response = Response::new(200, "OK");
        let body = format!("Successfully uploaded {} files", file_count);
        response.set_header("Content-Type", "text/plain");
        response.set_body_string(&body);
        response
    }

    // Create a file upload error response
    pub fn file_upload_error(status_code: u16, message: &str) -> Self {
        let mut response = Response::new(status_code, status_code_to_text(status_code));
        let body = message.to_string();
        response.set_header("Content-Type", "text/plain");
        response.set_body_string(&body);
        response
    }

    // Convert response to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response_text = String::new();

        // Status line
        writeln!(response_text, "HTTP/1.1 {} {}\r", self.status_code, self.status_text).unwrap();

        // Headers
        for (key, value) in self.headers.iter() {
            writeln!(response_text, "{}: {}\r", key, value).unwrap();
        }

        // Empty line to separate headers from body
        writeln!(response_text, "\r").unwrap();

        // Combine headers and body
        let mut response_bytes = response_text.into_bytes();
        response_bytes.extend_from_slice(&self.body);
        response_bytes
    }
}

// Helper function to convert status code to text
fn status_code_to_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        _ => "Unknown",
    }
}