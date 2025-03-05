use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Headers {
    headers: HashMap<String, String>,
}

impl Headers {
    pub fn new() -> Self {
        Headers {
            headers: HashMap::new(),
        }
    }

    pub fn add(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_lowercase(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.headers.get(&key.to_lowercase())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.headers.contains_key(&key.to_lowercase())
    }

    pub fn parse(header_lines: &[&str]) -> Self {
        let mut headers = Headers::new();
        for line in header_lines {
            if line.is_empty() {
                continue;
            }
            if let Some(colon_idx) = line.find(':') {
                let key = &line[0..colon_idx].trim();
                let value = &line[(colon_idx + 1)..].trim();
                headers.add(key, value);
            }
        }
        headers
    }

    // Method to check if content is multipart form data
    pub fn is_multipart_form_data(&self) -> bool {
        if let Some(content_type) = self.get("content-type") {
            content_type.starts_with("multipart/form-data")
        } else {
            false
        }
    }

    // Method to get boundary from content-type header
    pub fn get_boundary(&self) -> Option<String> {
        self.get("content-type").and_then(|content_type| {
            if let Some(boundary_part) = content_type.split("boundary=").nth(1) {
                Some(boundary_part.trim_matches('"').to_string())
            } else {
                None
            }
        })
    }

    // Get content length as u64
    pub fn get_content_length(&self) -> Option<u64> {
        self.get("content-length")
            .and_then(|length| length.parse::<u64>().ok())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.headers.iter()
    }
}