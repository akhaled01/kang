use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;

use crate::http::Request;
use crate::info;
use crate::error;

pub const MAX_EVENTS: usize = 1024;

/// TCP listening socket using the kqueue interface.
///
/// It contains a non-blocking listener, a kqueue file descriptor, and a map of connected clients.
/// Each server spawned by kang has its own kqueue listener.
#[derive(Debug)]
pub struct EpollListener {
    pub epoll_fd: RawFd, // kqueue fd
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

        if unsafe {
            libc::kevent(
                kq,
                &changes,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        } < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(EpollListener {
            epoll_fd: kq,
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

                    // Monitor for read and write events
                    let changes = [
                        libc::kevent {
                            ident: fd as usize,
                            filter: libc::EVFILT_READ as i16,
                            flags: libc::EV_ADD | libc::EV_ENABLE,
                            fflags: 0,
                            data: 0,
                            udata: ptr::null_mut(),
                        },
                        libc::kevent {
                            ident: fd as usize,
                            filter: libc::EVFILT_WRITE as i16,
                            flags: libc::EV_ADD | libc::EV_ENABLE,
                            fflags: 0,
                            data: 0,
                            udata: ptr::null_mut(),
                        },
                    ];

                    if unsafe {
                        libc::kevent(
                            global_epoll_fd,
                            changes.as_ptr(),
                            2,
                            ptr::null_mut(),
                            0,
                            ptr::null(),
                        )
                    } < 0 {
                        return Err(io::Error::last_os_error());
                    }

                    info!("Accepted connection from {:?} fd={}", addr, fd);
                    self.connections.insert(fd, stream);
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No more connections to accept
                    info!("No more connections to accept");
                    break;
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    return Err(e);
                },
            }
        }
        Ok(())
    }

    pub fn handle_connection(&mut self, fd: RawFd) -> io::Result<Request> {
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
                    info!("Received {} bytes from fd={}", n, fd);
                    buffer.extend_from_slice(&temp_buf[..n]);

                    // Debug: Print received data
                    if let Ok(data) = String::from_utf8(buffer.clone()) {
                        info!("Received data: {}", data);
                    }

                    // Try to parse what we have so far
                    match crate::http::Request::parse(&buffer) {
                        Ok(request) => {
                            info!("Successfully parsed HTTP request for fd={}", fd);
                            return Ok(request);
                        }
                        Err(e) => {
                            info!("Failed to parse request: {} for fd={}", e, fd);
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    info!("WouldBlock on fd={}, buffer size={}", fd, buffer.len());
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
            info!("Incomplete request on fd={}, waiting for more data", fd);
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Incomplete request",
            ));
        }

        info!("No data received on fd={}", fd);
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "No valid request received",
        ))
    }

    pub fn send_bytes(&self, bytes: Vec<u8>, fd: RawFd) -> io::Result<()> {
        let mut stream = self.connections.get(&fd).unwrap();
        stream.write_all(&bytes)?;
        Ok(())
    }

    pub fn remove_connection(&mut self, fd: RawFd, global_epoll_fd: RawFd) -> io::Result<()> {
        let changes = [
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_READ as i16,
                flags: libc::EV_DELETE,
                fflags: 0,
                data: 0,
                udata: ptr::null_mut(),
            },
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_WRITE as i16,
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
                2,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        } < 0 {
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
        info!("Kqueue listener shutting down");
    }
}
