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
                    // info!("Received {} bytes from fd={}", n, fd);
                    buffer.extend_from_slice(&temp_buf[..n]);

                    // Try to parse what we have so far
                    match Request::parse(&buffer) {
                        Ok(request) => {
                            // info!("Successfully parsed HTTP request for fd={}", fd);
                            return Ok(request);
                        }
                        Err(e) => {
                            warn!("Failed to parse request: {} for fd={}", e, fd);
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // We've read everything available for now
                    if buffer.len() > 0 {
                        debug!("Parsing request: {}", String::from_utf8_lossy(&buffer));
                        // Try to parse what we have
                        match Request::parse(&buffer) {
                            Ok(request) => {
                                // Check if we have the full body based on Content-Length
                                if let Some(content_length) = request.headers().get("Content-Length") {
                                    if let Ok(expected_len) = content_length.parse::<usize>() {
                                        let headers_end = find_headers_end(&buffer).unwrap_or(0);
                                        let actual_body_len = buffer.len() - headers_end - 4; // -4 for CRLFCRLF
                                        
                                        if actual_body_len >= expected_len {
                                            debug!("Got complete request with body of {} bytes", actual_body_len);
                                            return Ok(request);
                                        } else {
                                            warn!("Incomplete body: got {} of {} bytes", actual_body_len, expected_len);
                                            break; // Keep reading
                                        }
                                    }
                                } else {
                                    // No Content-Length header, assume request is complete
                                    return Ok(request);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse request: {} for fd={}", e, fd);
                                break;
                            }
                        }
                    }
                    break;
                }
                Err(e) => {
                    error!("Read error on fd={}: {}", fd, e);
                    return Err(e);
                }
            }
        }

        // If we have data but couldn't parse a complete request, keep waiting
        if !buffer.is_empty() {
            warn!("Incomplete request on fd={}, waiting for more data", fd);
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
