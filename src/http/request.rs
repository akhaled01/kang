use std::io;
use std::str;

use crate::debug;
use crate::http::headers::Headers;
use crate::http::upload::MultipartFormData;

use crate::http::methods::Method;
use crate::warn;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    path: String,
    query_params: HashMap<String, String>,
    version: String,
    headers: Headers,
    body: Vec<u8>,
    chunked: bool,
    keep_alive: bool,
}

impl Request {
    fn parse_query_params(path: &str) -> (String, HashMap<String, String>) {
        let mut params = HashMap::new();
        if let Some((base_path, query)) = path.split_once('?') {
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    params.insert(
                        urlencoding::decode(key)
                            .unwrap_or_else(|_| key.into())
                            .into_owned(),
                        urlencoding::decode(value)
                            .unwrap_or_else(|_| value.into())
                            .into_owned(),
                    );
                }
            }
            (base_path.to_string(), params)
        } else {
            (path.to_string(), params)
        }
    }

    pub fn query_param(&self, key: &str) -> Option<&String> {
        self.query_params.get(key)
    }

    pub fn query_params(&self) -> &HashMap<String, String> {
        &self.query_params
    }

    pub fn is_chunked(&self) -> bool {
        self.chunked
    }

    pub fn is_keep_alive(&self) -> bool {
        self.keep_alive
    }

    pub fn new(method: Method, path: &str, version: &str) -> Self {
        let (path, query_params) = Self::parse_query_params(path);
        Request {
            method,
            path,
            query_params,
            version: version.to_string(),
            headers: Headers::new(),
            body: Vec::new(),
            chunked: false,
            keep_alive: false,
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

    pub fn parse(raw_request: &[u8]) -> io::Result<Self> {
        debug!("Parsing request: {}", String::from_utf8_lossy(raw_request));
        // First, find the end of headers (double CRLF)
        let headers_end = match find_headers_end(raw_request) {
            Some(end) => end,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid request format",
                ))
            }
        };

        // Parse only the headers section as UTF-8
        let headers_bytes = &raw_request[0..headers_end];
        let headers_str = match std::str::from_utf8(headers_bytes) {
            Ok(s) => s,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid UTF-8 in headers",
                ))
            }
        };

        // Split into lines and parse the request line
        let lines: Vec<&str> = headers_str.split("\r\n").collect();
        if lines.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty request"));
        }

        // Parse the request line
        let request_parts: Vec<&str> = lines[0].split_whitespace().collect();
        if request_parts.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid request line",
            ));
        }

        let method = Method::from_str(request_parts[0]);
        let path = urlencoding::decode(request_parts[1]).unwrap().into_owned();
        let version = request_parts[2];
        let mut request = Request::new(method, &path, version);

        // Parse headers
        if lines.len() > 1 {
            let headers = Headers::parse(&lines[1..]);
            request.set_headers(headers.clone());
            // Check transfer encoding and connection headers
            request.chunked = headers
                .get("Transfer-Encoding")
                .map(|v| v.to_lowercase().contains("chunked"))
                .unwrap_or(false);

            request.keep_alive = match headers.get("Connection") {
                Some(conn) => conn.to_lowercase().contains("keep-alive"),
                None => request.version.contains("1.1"), // HTTP/1.1 defaults to keep-alive
            };
        }

        // Set the body as raw bytes (don't try to parse as UTF-8)
        if raw_request.len() > headers_end + 4 {
            // +4 for the CRLFCRLF
            request.set_body(raw_request[headers_end + 4..].to_vec());
        }
        Ok(request)
    }

    // Helper function to find the end of headers (double CRLF sequence)
    fn _find_headers_end(bytes: &[u8]) -> Option<usize> {
        for i in 0..bytes.len() - 3 {
            if bytes[i] == b'\r'
                && bytes[i + 1] == b'\n'
                && bytes[i + 2] == b'\r'
                && bytes[i + 3] == b'\n'
            {
                return Some(i);
            }
        }
        None
    }

    // Method to check if request contains a file upload
    pub fn has_file_upload(&self) -> bool {
        // debug!("Checking if request contains a file upload");
        // debug!("Method: {:?}", self.method);
        // debug!("Headers: {:?}", self.headers);
        // debug!("Body: {:?}", self.body);

        if !self.headers.is_multipart_form_data() {
            warn!("Request is not a multipart/form-data request");
            return false;
        }

        if self.body.is_empty() {
            warn!("Request body is empty");
            return false;
        }

        matches!(self.method, Method::POST)
    }

    // Parse the multipart form data for file uploads
    pub fn parse_multipart_form_data(&self) -> io::Result<MultipartFormData> {
        if !self.has_file_upload() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Not a file upload request",
            ));
        }

        let boundary = match self.headers.get_boundary() {
            Some(boundary) => boundary,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Missing boundary in multipart/form-data",
                ))
            }
        };
        MultipartFormData::parse(&self.body, &boundary)
    }
}

// Helper function to find the end of headers (double CRLF sequence)
fn find_headers_end(bytes: &[u8]) -> Option<usize> {
    for i in 0..bytes.len() - 3 {
        if bytes[i] == b'\r'
            && bytes[i + 1] == b'\n'
            && bytes[i + 2] == b'\r'
            && bytes[i + 3] == b'\n'
        {
            return Some(i);
        }
    }
    None
}
