use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use crate::{debug, info};
use crate::utils::parse_size;

pub struct UploadHandler {
    max_body_size: u64,
    upload_dir: String,
}

#[derive(Debug)]
pub struct UploadedFile {
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub content: Vec<u8>,
}

#[derive(Debug)]
pub struct MultipartFormData {
    pub fields: HashMap<String, String>,
    pub files: Vec<UploadedFile>,
}

impl UploadHandler {
    pub fn new(max_body_size: &str, upload_dir: &str) -> Self {
        let size_limit = parse_size(max_body_size).unwrap_or(10_000_000); // Default 10MB if parsing fails
        UploadHandler {
            max_body_size: size_limit,
            upload_dir: upload_dir.to_string(),
        }
    }

    pub fn handle_upload(&self, multipart_data: &MultipartFormData) -> io::Result<Vec<String>> {
        let mut saved_files = Vec::new();
        for file in &multipart_data.files {
            // Check file size against max body size
            if file.content.len() as u64 > self.max_body_size {
                return Err(io::Error::new(io::ErrorKind::InvalidData,
                    format!("File '{}' exceeds maximum allowed size", file.filename)));
            }
            // Generate file path
            let file_path = format!("{}/{}", self.upload_dir, file.filename);
            // Save file
            self.save_file(&file_path, &file.content)?;
            saved_files.push(file.filename.clone());
            info!("File uploaded successfully: {}", file.filename);
        }
        Ok(saved_files)
    }

    fn save_file(&self, path: &str, content: &[u8]) -> io::Result<()> {
        // Ensure the upload directory exists
        if let Some(parent) = Path::new(path).parent() {
            debug!("Creating directory: {:?}", parent);
            fs::create_dir_all(parent)?;
        }
        debug!("Writing file to: {}", path);
        let mut file = File::create(path)?;
        file.write_all(content)?;
        Ok(())
    }
}

impl MultipartFormData {
    pub fn parse(body: &[u8], boundary: &str) -> io::Result<Self> {
        let full_boundary = format!("--{}", boundary);
        let full_boundary_bytes = full_boundary.as_bytes();
        let end_boundary = format!("--{}--", boundary);
        let _end_boundary_bytes = end_boundary.as_bytes();

        let mut fields = HashMap::new();
        let mut files = Vec::new();

        // Find all boundary positions in the body
        let mut boundary_positions = Vec::new();
        let mut pos = 0;

        while pos < body.len() {
            if let Some(idx) = find_subsequence(&body[pos..], full_boundary_bytes) {
                boundary_positions.push(pos + idx);
                pos = pos + idx + full_boundary_bytes.len();
            } else {
                break;
            }
        }

        // Process each part between boundaries
        for i in 0..boundary_positions.len() - 1 {
            let start = boundary_positions[i] + full_boundary_bytes.len();
            let end = boundary_positions[i + 1];

            // Skip the first CRLF after the boundary
            if start + 2 <= body.len() && &body[start..start+2] == b"\r\n" {
                let start = start + 2;
                // Find the end of headers (double CRLF)
                if let Some(header_end) = find_subsequence(&body[start..end], b"\r\n\r\n") {
                    let header_end = start + header_end;
                    // Parse headers as text (headers are always text)
                    let headers_text = String::from_utf8_lossy(&body[start..header_end]);
                    // Extract content (binary safe)
                    let content_start = header_end + 4; // Skip double CRLF
                    // Find if the content ends with \r\n
                    let content_end = if end >= 2 && &body[end-2..end] == b"\r\n" {
                        end - 2
                    } else {
                        end
                    };

                    let content = &body[content_start..content_end];
                    // Extract field name, filename and content-type
                    let field_name = Self::extract_field_name(&headers_text);
                    let filename = Self::extract_filename(&headers_text);
                    let content_type = Self::extract_content_type(&headers_text)
                        .unwrap_or_else(|| "text/plain".to_string());
                    if let Some(name) = field_name {
                        if let Some(filename) = filename {
                            // This is a file (binary safe)
                            files.push(UploadedFile {
                                name: name.to_string(),
                                filename: filename.to_string(),
                                content_type,
                                content: content.to_vec(),
                            });
                        } else {
                            // This is a regular text field
                            let field_value = String::from_utf8_lossy(content).to_string();
                            fields.insert(name.to_string(), field_value);
                        }
                    }
                }
            }
        }
        Ok(MultipartFormData { fields, files })
    }

    fn extract_field_name(headers: &str) -> Option<&str> {
        for line in headers.lines() {
            if line.contains("Content-Disposition:") && line.contains("name=") {
                return line.split("name=").nth(1)
                    .and_then(|s| s.split('"').nth(1));
            }
        }
        None
    }

    fn extract_filename(headers: &str) -> Option<&str> {
        for line in headers.lines() {
            if line.contains("Content-Disposition:") && line.contains("filename=") {
                return line.split("filename=").nth(1)
                    .and_then(|s| s.split('"').nth(1));
            }
        }
        None
    }

    fn extract_content_type(headers: &str) -> Option<String> {
        for line in headers.lines() {
            if line.contains("Content-Type:") {
                return line.split(':').nth(1)
                    .map(|s| s.trim().to_string());
            }
        }
        None
    }
}

// Helper function to find a subsequence in a byte slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }

    'outer: for i in 0..=haystack.len() - needle.len() {
        for (j, &b) in needle.iter().enumerate() {
            if haystack[i + j] != b {
                continue 'outer;
            }
        }
        return Some(i);
    }

    None
}