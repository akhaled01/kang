#[cfg(target_os = "linux")]
mod epoll;
mod route;
#[cfg(target_os = "macos")]
mod kqueue;
pub mod server;
pub mod listener;

#[cfg(target_os = "linux")]
pub use epoll::EpollListener;
#[cfg(target_os = "macos")]
pub use kqueue::KqueueListener;
pub use server::Server;
