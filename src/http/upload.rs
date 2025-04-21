use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use crate::utils::parse_size;
use crate::{debug, info, warn};

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
        // let mut saved_files = Vec::new();
        // for file in &multipart_data.files {
        //     // Check file size against max body size
        //     if file.content.len() as u64 > self.max_body_size {
        //         return Err(io::Error::new(
        //             io::ErrorKind::InvalidData,
        //             format!("File '{}' exceeds maximum allowed size", file.filename),
        //         ));
        //     }
        //     // Generate file path
        //     //let file_path = format!("{}/{}", self.upload_dir, file.filename);
        //     let file_path = format!("{}uploads/{}", self.upload_dir, file.filename);
        //     // Save file
        //     self.save_file(&file_path, &file.content)?;
        //     saved_files.push(file.filename.clone());
        //     info!("File uploaded successfully: {}", file.filename);
        // }
        // Ok(saved_files)
        let mut saved_files = Vec::new();
        // First ensure uploads directory exists
        let uploads_dir = format!("{}uploads", self.upload_dir);
        fs::create_dir_all(&uploads_dir)?;

        for file in &multipart_data.files {
            // Check file size against max body size
            if file.content.len() as u64 > self.max_body_size {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("File '{}' exceeds maximum allowed size", file.filename),
                ));
            }

            // Generate file path
            let file_path = format!("{}uploads/{}", self.upload_dir, file.filename);
            // Save file
            self.save_file(&file_path, &file.content)?;
            saved_files.push(file.filename.clone());
            info!("File uploaded successfully: {}", file.filename);
        }
        Ok(saved_files)
    }

    fn save_file(&self, path: &str, content: &[u8]) -> io::Result<()> {
        // // Ensure the upload directory exists
        // if let Some(parent) = Path::new(path).parent() {
        //     debug!("Creating directory: {:?}", parent);
        //     fs::create_dir_all(parent)?;
        // }
        // debug!("Writing file to: {}", path);
        // let mut file = File::create(path)?;
        // file.write_all(content)?;
        // Ok(())
        // Ensure the upload directory exists
        if let Some(parent) = Path::new(path).parent() {
            debug!("Creating directory: {:?}", parent);
            fs::create_dir_all(parent)?;
        }
        debug!("Writing file to: {} ({} bytes)", path, content.len());

        // Create file with explicit options to ensure binary safety
        let mut file = File::create(path)?;
        // Write all content in one operation
        file.write_all(content)?;
        // Ensure data is flushed to disk
        file.flush()?;
        // Verify file was written correctly
        let metadata = fs::metadata(path)?;
        if metadata.len() != content.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("File size mismatch: expected {} bytes, got {} bytes", 
                        content.len(), metadata.len())
            ));
        }
        info!("File saved: {} (size: {} bytes)", path, metadata.len());
        Ok(())
    }
}

impl MultipartFormData {
    pub fn parse(body: &[u8], boundary: &str) -> io::Result<Self> {
        if body.is_empty() {
            warn!("Empty request body");
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Empty request body",
            ));
        }

        debug!("Request body: {}", String::from_utf8_lossy(body));

        let full_boundary = format!("--{}", boundary);
        let full_boundary_bytes = full_boundary.as_bytes();
        let end_boundary = format!("--{}--", boundary);
        let end_boundary_bytes = end_boundary.as_bytes();

        let mut fields = HashMap::new();
        let mut files = Vec::new();

        // Find all boundary positions in the body
        let mut boundary_positions = Vec::new();
        let mut pos = 0;

        while pos < body.len() {
            if let Some(idx) = find_subsequence(&body[pos..], full_boundary_bytes) {
                let abs_pos = pos + idx;
                // Check if this is an end boundary
                if abs_pos + full_boundary_bytes.len() + 2 <= body.len()
                    && &body[abs_pos + full_boundary_bytes.len()
                        ..abs_pos + full_boundary_bytes.len() + 2]
                        == b"--"
                {
                    break; // End boundary found
                }
                boundary_positions.push(abs_pos);
                pos = abs_pos + full_boundary_bytes.len();
            } else {
                break;
            }
        }

        // Process each part between boundaries
        for i in 0..boundary_positions.len() {
            let start = boundary_positions[i] + full_boundary_bytes.len();
            let end = if i + 1 < boundary_positions.len() {
                boundary_positions[i + 1]
            } else {
                // For the last part, look for end boundary or use body length
                if let Some(end_idx) = find_subsequence(&body[start..], end_boundary_bytes) {
                    start + end_idx
                } else {
                    body.len()
                }
            };

            // Skip if start would exceed end
            if start >= end || start >= body.len() {
                continue;
            }

            // Skip the first CRLF after the boundary
            if start + 2 <= body.len() && &body[start..start + 2] == b"\r\n" {
                let content_start = start + 2;
                if let Some(header_end) = find_subsequence(&body[content_start..end], b"\r\n\r\n") {
                    let header_end = content_start + header_end;
                    // Ensure header_end is within bounds
                    if header_end + 4 <= end {
                        let headers_text =
                            String::from_utf8_lossy(&body[content_start..header_end]);
                        let content_start = header_end + 4;
                        // let content_end = if end >= 2 && &body[end - 2..end] == b"\r\n" {
                        //     end - 2
                        // } else {
                        //     end
                        // };

                        // More precise content end calculation
                        let content_end = if end >= 2 && &body[end - 2..end] == b"\r\n" {
                            end - 2
                        } else {
                            end
                        };

                        if content_end > content_start {
                            let content = &body[content_start..content_end];
                            // Rest of the processing remains the same
                            // let field_name = Self::extract_field_name(&headers_text);
                            // let filename = Self::extract_filename(&headers_text);
                            // let content_type = Self::extract_content_type(&headers_text)
                            //     .unwrap_or_else(|| "text/plain".to_string());
                            let field_name = Self::extract_field_name(&headers_text);
                            let filename = Self::extract_filename(&headers_text);
                            let content_type = Self::extract_content_type(&headers_text)
                                .unwrap_or_else(|| "application/octet-stream".to_string());
                            // if let Some(name) = field_name {
                            //     if let Some(filename) = filename {
                            //         // This is a file (binary safe)
                            //         files.push(UploadedFile {
                            //             name: name.to_string(),
                            //             filename: filename.to_string(),
                            //             content_type,
                            //             content: content.to_vec(),
                            //         });
                            //     } else {
                            //         // This is a regular text field
                            //         let field_value = String::from_utf8_lossy(content).to_string();
                            //         fields.insert(name.to_string(), field_value);
                            //     }
                            // }
                            if let Some(name) = field_name {
                                if let Some(filename) = filename {
                                    // This is a file (binary safe)
                                    debug!("Found file: {} ({}, {} bytes)", 
                                           filename, content_type, content.len());
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
            }
        }

        if files.is_empty() && fields.is_empty() {
            warn!("Failed to parse any fields or files from multipart data");
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Failed to parse any fields or files from multipart data",
            ));
        }

        Ok(MultipartFormData { fields, files })
    }

    fn extract_field_name(headers: &str) -> Option<&str> {
        for line in headers.lines() {
            if line.contains("Content-Disposition:") && line.contains("name=") {
                return line.split("name=").nth(1).and_then(|s| s.split('"').nth(1));
            }
        }
        None
    }

    fn extract_filename(headers: &str) -> Option<&str> {
        for line in headers.lines() {
            if line.contains("Content-Disposition:") && line.contains("filename=") {
                return line
                    .split("filename=")
                    .nth(1)
                    .and_then(|s| s.split('"').nth(1));
            }
        }
        None
    }

    fn extract_content_type(headers: &str) -> Option<String> {
        for line in headers.lines() {
            if line.contains("Content-Type:") {
                return line.split(':').nth(1).map(|s| s.trim().to_string());
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
