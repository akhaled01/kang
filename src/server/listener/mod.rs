#[cfg(target_os = "linux")]
mod epoll;
#[cfg(target_os = "macos")]
mod kqueue;
mod listener;

#[cfg(target_os = "linux")]
pub use epoll::EpollListener;
#[cfg(target_os = "macos")]
pub use kqueue::KqueueListener;

pub use listener::{Listener, MAX_EVENTS};
