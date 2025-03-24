use std::fmt::Write;

use crate::http::Headers;

use super::{cookies::Cookie, status::StatusCode};

#[derive(Debug)]
pub struct Response {
    status_code: StatusCode,
    status_text: String,
    headers: Headers,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: StatusCode) -> Self {
        let status_text = status_code.to_text();

        let mut response = Response {
            status_code,
            status_text,
            headers: Headers::new(),
            body: Vec::new(),
        };

        // Add default headers
        response.headers.add("Server", "Kang");
        response.headers.add("Connection", "close");

        response
    }

    pub fn status_code(&self) -> &StatusCode {
        &self.status_code
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

    pub fn set_cookie(&mut self, cookie: Cookie) {
        self.headers.add("Set-Cookie", &cookie.to_string());
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = body;
        self.set_header("Content-Length", &self.body.len().to_string());
    }

    pub fn set_body_string(&mut self, body: &str) {
        self.set_body(body.as_bytes().to_vec());
    }

    // Convert response to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response_text = String::new();

        // Status line
        writeln!(
            response_text,
            "HTTP/1.1 {} {}\r",
            self.status_code as u16,
            self.status_text
        )
        .unwrap();

        self.headers.iter().for_each(|(key, value)| {
            writeln!(response_text, "{}: {}\r", key, value).unwrap();
        });

        // Empty line to separate headers from body
        writeln!(response_text, "\r").unwrap();

        // Combine headers and body
        let mut response_bytes = response_text.into_bytes();
        response_bytes.extend_from_slice(&self.body);
        response_bytes
    }
}
