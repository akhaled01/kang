#[cfg(target_os = "linux")]
mod epoll;
mod route;
#[cfg(target_os = "macos")]
mod kqueue;
pub mod server;

#[cfg(target_os = "linux")]
pub use epoll::EpollListener;
#[cfg(target_os = "macos")]
pub use kqueue::EpollListener;
pub use server::Server;
