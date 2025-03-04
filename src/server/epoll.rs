use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};

use crate::info;

pub const MAX_EVENTS: usize = 1024;

/// The server struct represents a TCP listening socket using the epoll interface.
///
/// It contains a non-blocking listener, an epoll file descriptor, and a map of connected clients.
#[derive(Debug)]
pub struct Server {
    pub epoll_fd: RawFd,
    pub listener: TcpListener,
    pub connections: HashMap<RawFd, TcpStream>,
}

impl Server {
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

        Ok(Server {
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
                        Err(e) => eprintln!("Accept error: {}", e),
                    }
                } else {
                    // Handle existing connection
                    match self.handle_connection(fd) {
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!("Connection error: {}", e);
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
                Ok((stream, _)) => {
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

                    info!("Accepted connection from fd={}", fd);

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
                    println!("Received {} bytes", n);
                    println!("{}", String::from_utf8_lossy(&buffer[..n]));
                    let response = "HTTP/1.1 200 OK\r\n\
                                  Content-Length: 13\r\n\
                                  \r\n\
                                  Hello, World!";
                    stream.write_all(response.as_bytes())?;
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
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        unsafe { libc::close(self.epoll_fd) };
    }
}
