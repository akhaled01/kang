use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::fs;
use std::path::Path;

use crate::{error, info, warn};

pub const MAX_EVENTS: usize = 1024;

/// TCP listening socket using the epoll interface.
///
/// It contains a non-blocking listener, an epoll file descriptor, and a map of connected clients.
/// Each server spawned by kang has its own epoll listener.
#[derive(Debug)]
pub struct EpollListener {
    pub epoll_fd: RawFd,
    pub listener: TcpListener,
    pub connections: HashMap<RawFd, TcpStream>,
}

impl EpollListener {
    /// Creates a new instance of the server.
    ///
    /// # Arguments
    /// * `addr` - The address to bind the server to.
    ///
    /// # Returns
    /// A new instance of the server.
    pub fn new(addr: &str) -> io::Result<Self> {
        // Create non-blocking listener
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;

        // Create epoll instance
        let epoll_fd = unsafe { libc::epoll_create1(0) };
        if epoll_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Add listener to epoll
        let mut event = libc::epoll_event {
            events: (libc::EPOLLIN | libc::EPOLLET) as u32,
            u64: listener.as_raw_fd() as u64,
        };

        // register listener file descriptor for epoll instance to
        // start monitoring it
        if unsafe {
            libc::epoll_ctl(
                epoll_fd,
                libc::EPOLL_CTL_ADD,
                listener.as_raw_fd(),
                &mut event,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }

        Ok(EpollListener {
            epoll_fd,
            listener,
            connections: HashMap::new(),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; MAX_EVENTS];

        loop {
            let nfds = unsafe {
                libc::epoll_wait(self.epoll_fd, events.as_mut_ptr(), MAX_EVENTS as i32, -1)
            };

            if nfds < 0 {
                return Err(io::Error::last_os_error());
            }

            for n in 0..nfds {
                let fd = events[n as usize].u64 as RawFd;

                if fd == self.listener.as_raw_fd() {
                    // Handle new connection
                    match self.accept_connection() {
                        Ok(_) => (),
                        Err(e) => error!("Accept error: {}", e),
                    }
                } else {
                    // Handle existing connection
                    match self.handle_connection(fd) {
                        Ok(_) => (),
                        Err(e) => {
                            warn!("Connection error: {}", e);
                            self.remove_connection(fd)?;
                        }
                    }
                }
            }
        }
    }

    pub fn accept_connection(&mut self) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    stream.set_nonblocking(true)?;
                    let fd = stream.as_raw_fd();

                    // Add to epoll
                    let mut event = libc::epoll_event {
                        events: (libc::EPOLLIN | libc::EPOLLET) as u32,
                        u64: fd as u64,
                    };

                    if unsafe {
                        libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event)
                    } < 0
                    {
                        return Err(io::Error::last_os_error());
                    }

                    info!("Accepted connection from {:?} fd={}", addr, fd);

                    self.connections.insert(fd, stream);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No more connections to accept
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn handle_connection(&mut self, fd: RawFd) -> io::Result<()> {
        let stream = self.connections.get_mut(&fd).unwrap();
        let mut buffer = [0; 4096];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    // Connection closed
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "Connection closed",
                    ));
                }
                Ok(n) => {
                    // Process HTTP request here
                    info!("Received {} bytes from fd={}", n, fd);
                    let request = match crate::http::Request::parse(&buffer[..n]) {
                        Ok(req) => req,
                        Err(e) => {
                            let mut response = crate::http::Response::new(400, "Bad Request");
                            response.set_header("Content-Type", "text/plain");
                            response.set_body_string(&format!("Bad request: {}", e));
                            stream.write_all(&response.to_bytes())?;
                            continue;
                        }
                    };

                    info!("Processing {} request to {}", request.method().as_str(), request.path());
                    // Generate response based on request
                    let response = if request.has_file_upload() {
                        // Handle file upload using our upload module
                        match request.parse_multipart_form_data() {
                            Ok(form_data) => {
                                //let upload_handler = crate::http::UploadHandler::new("10M", "./static/uploads");
                                let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "./static/uploads".to_string());
                                let upload_handler = crate::http::UploadHandler::new("10M", &upload_dir);
                                match upload_handler.handle_upload(&form_data) {
                                    Ok(files) => {
                                        info!("Successfully uploaded {} files", files.len());
                                        crate::http::Response::file_upload_success(files.len())
                                    },
                                    Err(e) => {
                                        error!("Upload error: {}", e);
                                        crate::http::Response::file_upload_error(500, &e.to_string())
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Failed to parse multipart form data: {}", e);
                                crate::http::Response::file_upload_error(400, &e.to_string())
                            }
                        }
                    } else {
                        // Handle static file requests
                        let path = request.path();
                        let mut file_path = format!("static{}", path);
                        
                        // If path ends with /, add index.html
                        if path.ends_with("/") {
                            file_path.push_str("index.html");
                        }
                        
                        // If file exists, serve it
                        if Path::new(&file_path).exists() && fs::metadata(&file_path).map(|m| m.is_file()).unwrap_or(false) {
                            let content_type = match Path::new(&file_path).extension().and_then(|e| e.to_str()) {
                                Some("html") => "text/html",
                                Some("css") => "text/css",
                                Some("js") => "application/javascript",
                                Some("jpg") | Some("jpeg") => "image/jpeg",
                                Some("png") => "image/png",
                                Some("gif") => "image/gif",
                                _ => "application/octet-stream",
                            };
                            
                            match fs::read(&file_path) {
                                Ok(content) => {
                                    info!("Serving static file: {}", file_path);
                                    let mut response = crate::http::Response::new(200, "OK");
                                    response.set_header("Content-Type", content_type);
                                    response.set_body(content);
                                    response
                                },
                                Err(e) => {
                                    error!("Failed to read file {}: {}", file_path, e);
                                    let mut response = crate::http::Response::new(500, "Internal Server Error");
                                    response.set_header("Content-Type", "text/plain");
                                    response.set_body_string(&format!("Failed to read file: {}", e));
                                    response
                                }
                            }
                        } else {
                            // File not found
                            warn!("File not found: {}", file_path);
                            let mut response = crate::http::Response::new(404, "Not Found");
                            response.set_header("Content-Type", "text/plain");
                            response.set_body_string(&format!("404 - File Not Found: {}", path));
                            response
                        }
                    };

                    // Send response
                    stream.write_all(&response.to_bytes())?;
                    info!("Response sent with status code {}", response.status_code());
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No more data to read
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn remove_connection(&mut self, fd: RawFd) -> io::Result<()> {
        if unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) }
            < 0
        {
            return Err(io::Error::last_os_error());
        }
        self.connections.remove(&fd);
        info!("Connection removed: fd={}", fd);
        Ok(())
    }
}

impl Drop for EpollListener {
    fn drop(&mut self) {
        unsafe { libc::close(self.epoll_fd) };
        info!("Epoll listener shutting down");
    }
}
