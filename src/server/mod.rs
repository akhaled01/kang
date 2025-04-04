#[cfg(target_os = "linux")]
mod epoll;
mod route;
#[cfg(target_os = "macos")]
mod kqueue;
pub mod server;
pub mod listener;

#[cfg(target_os = "linux")]
pub use epoll::EpollListener;
pub use server::Server;
