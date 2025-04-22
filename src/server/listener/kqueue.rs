#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::io::{self, Read, Write};
#[cfg(target_os = "macos")]
use std::net::{TcpListener, TcpStream};
#[cfg(target_os = "macos")]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(target_os = "macos")]
use std::ptr;

#[cfg(target_os = "macos")]
use crate::error;
#[cfg(target_os = "macos")]
use crate::http::Request;
#[cfg(target_os = "macos")]
use crate::info;
use crate::{debug, warn};

use super::listener::Listener;

// Find the end of headers in a request (double CRLF)
fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    for i in 0..buffer.len() - 3 {
        if buffer[i..i + 4] == [b'\r', b'\n', b'\r', b'\n'] {
            return Some(i);
        }
    }
    None
}

#[cfg(target_os = "macos")]
/// TCP listening socket using the kqueue interface.
///
/// It contains a non-blocking listener, a kqueue file descriptor, and a map of connected clients.
/// Each server spawned by kang has its own kqueue listener.
#[derive(Debug)]
pub struct KqueueListener {
    pub kqueue_fd: RawFd, // kqueue fd
    pub listener: TcpListener,
    pub connections: HashMap<RawFd, TcpStream>,
}

#[cfg(target_os = "macos")]
impl KqueueListener {
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

        // Create kqueue instance
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }

        // Add listener to kqueue
        let changes = libc::kevent {
            ident: listener.as_raw_fd() as usize,
            filter: libc::EVFILT_READ as i16,
            flags: libc::EV_ADD | libc::EV_ENABLE,
            fflags: 0,
            data: 0,
            udata: ptr::null_mut(),
        };

        if unsafe { libc::kevent(kq, &changes, 1, ptr::null_mut(), 0, ptr::null()) } < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(KqueueListener {
            kqueue_fd: kq,
            listener,
            connections: HashMap::new(),
        })
    }
}

#[cfg(target_os = "macos")]
impl Listener for KqueueListener {
    fn new(addr: &str) -> io::Result<Self> {
        KqueueListener::new(addr)
    }
    fn accept_connection(&mut self, global_kqueue_fd: RawFd) -> io::Result<()> {
        // Only try once, since we're in non-blocking mode and got a read event
        match self.listener.accept() {
            Ok((stream, _addr)) => {
                stream.set_nonblocking(true)?;
                let fd = stream.as_raw_fd();

                // Monitor for read events only
                let changes = [
                    libc::kevent {
                        ident: fd as usize,
                        filter: libc::EVFILT_READ as i16,
                        flags: libc::EV_ADD | libc::EV_ENABLE,
                        fflags: 0,
                        data: 0,
                        udata: ptr::null_mut(),
                    },
                ];

                if unsafe {
                    libc::kevent(
                        global_kqueue_fd,
                        changes.as_ptr(),
                        1,
                        ptr::null_mut(),
                        0,
                        ptr::null(),
                    )
                } < 0
                {
                    return Err(io::Error::last_os_error());
                }

                // info!("Accepted connection from {:?} fd={}", addr, fd);
                self.connections.insert(fd, stream);
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No more connections to accept
                // info!("No more connections to accept");
                return Ok(());
            }
            Err(e) => {
                error!("Accept error: {}", e);
                return Err(e);
            }
        }
    }

    fn handle_connection(&mut self, fd: RawFd) -> io::Result<Request> {
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

    fn send_bytes(&self, bytes: Vec<u8>, fd: RawFd) -> io::Result<()> {
        let mut stream = self.connections.get(&fd).unwrap();
        stream.write_all(&bytes)?;
        Ok(())
    }

    fn get_port(&self) -> u16 {
        self.listener.local_addr().unwrap().port()
    }

    fn remove_connection(&mut self, fd: RawFd, global_epoll_fd: RawFd) -> io::Result<()> {
        let changes = [
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_READ as i16,
                flags: libc::EV_DELETE,
                fflags: 0,
                data: 0,
                udata: ptr::null_mut(),
            },
        ];

        if unsafe {
            libc::kevent(
                global_epoll_fd,
                changes.as_ptr(),
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }

        self.connections.remove(&fd);
        // info!("Connection removed: fd={}", fd);
        Ok(())
    }

    fn get_id(&self) -> RawFd {
        self.listener.as_raw_fd()
    }
}

#[cfg(target_os = "macos")]
impl Drop for KqueueListener {
    fn drop(&mut self) {
        unsafe { libc::close(self.kqueue_fd) };
        // info!("Kqueue listener shutting down");
    }
}
