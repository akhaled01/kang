use std::{io, os::fd::RawFd};

use crate::http::Request;

#[cfg(target_os = "linux")]
use crate::server::listener::epoll::EpollListener;

pub const MAX_EVENTS: usize = 1024;

/// Trait for a listener. A listener is a TCP listener that handles connections using I/O Multiplexing
/// On macOS, it uses the `kqueue` interface, and on Linux, it uses the `epoll` interface.
pub trait Listener: Send + Sync {
    fn new(addr: &str) -> io::Result<Self>
    where
        Self: Sized;
    fn get_id(&self) -> RawFd;
    fn accept_connection(&mut self, global_epoll_fd: RawFd) -> io::Result<()>;
    fn handle_connection(&mut self, fd: RawFd) -> io::Result<Request>;
    fn send_bytes(&self, bytes: Vec<u8>, fd: RawFd) -> io::Result<()>;
    fn remove_connection(&mut self, fd: RawFd, global_epoll_fd: RawFd) -> io::Result<()>;
    fn get_port(&self) -> u16;
}

#[cfg(target_os = "linux")]
impl Listener for EpollListener {
    fn new(addr: &str) -> io::Result<Self> {
        EpollListener::new(addr)
    }

    fn get_id(&self) -> RawFd {
        self.epoll_fd
    }

    fn accept_connection(&mut self, global_epoll_fd: RawFd) -> io::Result<()> {
        self.accept_connection(global_epoll_fd)
    }

    fn handle_connection(&mut self, fd: RawFd) -> io::Result<Request> {
        self.handle_connection(fd)
    }

    fn send_bytes(&self, bytes: Vec<u8>, fd: RawFd) -> io::Result<()> {
        self.send_bytes(bytes, fd)
    }

    fn remove_connection(&mut self, fd: RawFd, global_epoll_fd: RawFd) -> io::Result<()> {
        self.remove_connection(fd, global_epoll_fd)
    }

    fn get_port(&self) -> u16 {
        self.listener.local_addr().port()
    }
}
