#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::io::{self, Read, Write};
#[cfg(target_os = "linux")]
use std::net::{TcpListener, TcpStream};
#[cfg(target_os = "linux")]
use std::os::unix::io::{AsRawFd, RawFd};

#[cfg(target_os = "linux")]
use crate::error;
#[cfg(target_os = "linux")]
use crate::http::Request;
#[cfg(target_os = "linux")]
use crate::info;

#[cfg(target_os = "linux")]
use libc::{
    epoll_create1, epoll_ctl, epoll_event, EPOLLET, EPOLLIN, EPOLLOUT, EPOLL_CTL_ADD, EPOLL_CTL_DEL,
};

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

    pub fn accept_connection(&mut self, global_epoll_fd: RawFd) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    stream.set_nonblocking(true)?;
                    let fd = stream.as_raw_fd();

                    // Monitor for read, write, and edge-triggered events
                    let mut event = libc::epoll_event {
                        events: (libc::EPOLLIN | libc::EPOLLOUT | libc::EPOLLET) as u32,
                        u64: fd as u64,
                    };

                    if unsafe {
                        libc::epoll_ctl(global_epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event)
                    } < 0
                    {
                        return Err(io::Error::last_os_error());
                    }

                    // info!("Accepted connection from {:?} fd={}", addr, fd);
                    self.connections.insert(fd, stream);
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No more connections to accept
                    // info!("No more connections to accept");
                    break;
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn handle_connection(&mut self, fd: RawFd) -> io::Result<Request> {
        let stream = self.connections.get_mut(&fd).unwrap();
        let mut buffer = Vec::new();
        let mut temp_buf = [0; 4096];
        let mut content_length: Option<usize> = None;
        let mut headers_end: Option<usize> = None;

        loop {
            match stream.read(&mut temp_buf) {
                Ok(0) => {
                    info!("Connection closed by peer fd={}", fd);
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "Connection closed",
                    ));
                }
                Ok(n) => {
                    buffer.extend_from_slice(&temp_buf[0..n]);

                    // If we haven't found the headers end yet, try to find it
                    if headers_end.is_none() {
                        if let Some(end) = find_headers_end(&buffer) {
                            headers_end = Some(end);
                            
                            // Parse headers to get Content-Length
                            if let Ok(headers_str) = std::str::from_utf8(&buffer[0..end]) {
                                let lines: Vec<&str> = headers_str.split("\r\n").collect();
                                if lines.len() > 1 {
                                    for line in &lines[1..] {
                                        if let Some((name, value)) = line.split_once(": ") {
                                            if name.eq_ignore_ascii_case("Content-Length") {
                                                if let Ok(len) = value.trim().parse::<usize>() {
                                                    debug!("Found Content-Length: {}", len);
                                                    content_length = Some(len);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // If we have the headers end and content length, check if we have the full body
                    if let (Some(end), Some(length)) = (headers_end, content_length) {
                        let total_length = end + 4 + length; // +4 for CRLFCRLF
                        let current_length = buffer.len();
                        debug!("Current length: {}, Total needed: {}", current_length, total_length);
                        
                        if current_length >= total_length {
                            // We have the complete request
                            debug!("Got complete request with body size: {}", length);
                            match Request::parse(&buffer) {
                                Ok(request) => return Ok(request),
                                Err(e) => return Err(e),
                            }
                        }
                        // Don't break on WouldBlock if we haven't received the full body
                        continue;
                    } else if headers_end.is_some() && content_length.is_none() {
                        // We have headers but no content length, try to parse
                        match Request::parse(&buffer) {
                            Ok(request) => return Ok(request),
                            Err(e) if e.kind() == io::ErrorKind::InvalidData => return Err(e),
                            Err(_) => (), // Continue reading
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Only break if we don't know how much data we need
                    // or if we have no data at all
                    if content_length.is_none() || buffer.is_empty() {
                        break;
                    }
                    // Otherwise, continue trying to read
                    continue;
                }
                Err(e) => {
                    error!("Read error on fd={}: {}", fd, e);
                    return Err(e);
                }
            }
        }

        // If we have data but couldn't parse a complete request, keep waiting
        if !buffer.is_empty() {
            if let (Some(end), Some(length)) = (headers_end, content_length) {
                let total_length = end + 4 + length;
                warn!("Incomplete request on fd={}, got {} of {} bytes", fd, buffer.len(), total_length);
            } else {
                warn!("Incomplete request on fd={}, waiting for more data", fd);
            }
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Incomplete request",
            ));
        }

        warn!("No data received on fd={}", fd);
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "No valid request received",
        ))
    }

    // writes a response to a specified fd
    pub fn send_bytes(&self, bytes: Vec<u8>, fd: RawFd) -> io::Result<()> {
        let mut stream = self.connections.get(&fd).unwrap();
        stream.write_all(&bytes)?;
        Ok(())
    }

    pub fn remove_connection(&mut self, fd: RawFd, global_epoll_fd: RawFd) -> io::Result<()> {
        if unsafe {
            libc::epoll_ctl(
                global_epoll_fd,
                libc::EPOLL_CTL_DEL,
                fd,
                std::ptr::null_mut(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        self.connections.remove(&fd);
        // info!("Connection removed: fd={}", fd);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for EpollListener {
    fn drop(&mut self) {
        unsafe { libc::close(self.epoll_fd) };
        // info!("Epoll listener shutting down");
    }
}
