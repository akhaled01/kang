use std::collections::HashMap;
use std::io::{self, Read};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};

use crate::http::Request;
use crate::info;

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

    pub fn accept_connection(&mut self) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    stream.set_nonblocking(true)?;
                    let fd = stream.as_raw_fd();

                    //* pass through epoll
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

    pub fn handle_connection(&mut self, fd: RawFd) -> io::Result<Request> {
        let stream = self.connections.get_mut(&fd).unwrap();
        let mut buffer = [0; 4096];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "Connection closed",
                    ));
                }
                Ok(n) => {
                    info!("Received {} bytes from fd={}", n, fd);

                    // Attempt to parse the HTTP request
                    let request = crate::http::Request::parse(&buffer[..n]).map_err(|e| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("Bad request: {}", e))
                    })?;

                    return Ok(request);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "No valid request received",
        ))
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
