use std::io;
use std::str;

use crate::http::headers::Headers;
use crate::http::upload::MultipartFormData;

#[derive(Debug, Clone)]
pub enum Method {
    GET,
    POST,
    DELETE,
    UNKNOWN(String),
}

impl Method {
    pub fn as_str(&self) -> &str {
        match self {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::DELETE => "DELETE",
            Method::UNKNOWN(s) => s,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "DELETE" => Method::DELETE,
            _ => Method::UNKNOWN(s.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    path: String,
    version: String,
    headers: Headers,
    body: Vec<u8>,
}

impl Request {
    pub fn new(method: Method, path: &str, version: &str) -> Self {
        Request {
            method,
            path: path.to_string(),
            version: version.to_string(),
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn set_headers(&mut self, headers: Headers) {
        self.headers = headers;
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = body;
    }

    pub fn append_body(&mut self, data: &[u8]) {
        self.body.extend_from_slice(data);
    }

    // pub fn parse(raw_request: &[u8]) -> io::Result<Self> {
    //     let request_str = match str::from_utf8(raw_request) {
    //         Ok(s) => s,
    //         Err(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8")),
    //     };

    //     let lines: Vec<&str> = request_str.split("\r\n").collect();
    //     if lines.is_empty() {
    //         return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty request"));
    //     }

    //     // Parse the request line
    //     let request_parts: Vec<&str> = lines[0].split_whitespace().collect();
    //     if request_parts.len() != 3 {
    //         return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid request line"));
    //     }

    //     let method = Method::from_str(request_parts[0]);
    //     let path = request_parts[1];
    //     let version = request_parts[2];
    //     let mut request = Request::new(method, path, version);

    //     // Find the end of headers
    //     let mut headers_end = 0;
    //     for (i, &line) in lines.iter().enumerate() {
    //         if line.is_empty() {
    //             headers_end = i;
    //             break;
    //         }
    //     }

    //     // Parse headers
    //     let headers = Headers::parse(&lines[1..headers_end]);
    //     request.set_headers(headers);

    //     // Parse body if present
    //     if headers_end < lines.len() - 1 {
    //         let body_str = lines[(headers_end + 1)..].join("\r\n");
    //         request.set_body(body_str.as_bytes().to_vec());
    //     }
    //     Ok(request)
    // }

    pub fn parse(raw_request: &[u8]) -> io::Result<Self> {
        // First, find the end of headers (double CRLF)
        let headers_end = match find_headers_end(raw_request) {
            Some(end) => end,
            None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid request format")),
        };

        // Parse only the headers section as UTF-8
        let headers_bytes = &raw_request[0..headers_end];
        let headers_str = match std::str::from_utf8(headers_bytes) {
            Ok(s) => s,
            Err(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in headers")),
        };

        // Split into lines and parse the request line
        let lines: Vec<&str> = headers_str.split("\r\n").collect();
        if lines.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty request"));
        }

        // Parse the request line
        let request_parts: Vec<&str> = lines[0].split_whitespace().collect();
        if request_parts.len() != 3 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid request line"));
        }

        let method = Method::from_str(request_parts[0]);
        let path = request_parts[1];
        let version = request_parts[2];
        let mut request = Request::new(method, path, version);

        // Parse headers
        if lines.len() > 1 {
            let headers = Headers::parse(&lines[1..]);
            request.set_headers(headers);
        }

        // Set the body as raw bytes (don't try to parse as UTF-8)
        if raw_request.len() > headers_end + 4 {  // +4 for the CRLFCRLF
            request.set_body(raw_request[headers_end + 4..].to_vec());
        }
        Ok(request)
    }

    // Helper function to find the end of headers (double CRLF sequence)
    fn find_headers_end(bytes: &[u8]) -> Option<usize> {
        for i in 0..bytes.len() - 3 {
            if bytes[i] == b'\r' && bytes[i + 1] == b'\n' && bytes[i + 2] == b'\r' && bytes[i + 3] == b'\n' {
                return Some(i);
            }
        }
        None
    }

    // Method to check if request contains a file upload
    pub fn has_file_upload(&self) -> bool {
        matches!(self.method, Method::POST) && self.headers.is_multipart_form_data()
    }

    // Parse the multipart form data for file uploads
    pub fn parse_multipart_form_data(&self) -> io::Result<MultipartFormData> {
        if !self.has_file_upload() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Not a file upload request"));
        }

        let boundary = match self.headers.get_boundary() {
            Some(boundary) => boundary,
            None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "Missing boundary in multipart/form-data")),
        };
        MultipartFormData::parse(&self.body, &boundary)
    }
}

// Helper function to find the end of headers (double CRLF sequence)
fn find_headers_end(bytes: &[u8]) -> Option<usize> {
    for i in 0..bytes.len() - 3 {
        if bytes[i] == b'\r' && bytes[i + 1] == b'\n' && bytes[i + 2] == b'\r' && bytes[i + 3] == b'\n' {
            return Some(i);
        }
    }
    None
}