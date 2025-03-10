use std::os::fd::RawFd;
use std::os::unix::io::AsRawFd;
use std::{collections::HashMap, io};

use crate::server::epoll::MAX_EVENTS;
use crate::{error, info, warn};

use super::EpollListener;

pub struct Server {
    pub listeners: HashMap<i32, EpollListener>,
    pub server_name: Vec<String>,
    pub host: String,
    pub ports: Vec<u16>,
    pub is_default: bool,
    pub routes: Vec<Route>,
    pub client_max_body_size: Option<String>,
}

pub struct Route {
    pub path: String,
    pub root: Option<String>,
    pub index: Option<String>,
    pub methods: Vec<String>,
    pub directory_listing: bool,
    pub redirect: Option<Redirect>,
    pub cgi: Option<HashMap<String, String>>,
    pub client_max_body_size: Option<String>,
}

pub struct Redirect {
    pub url: String,
    pub code: u16,
}

impl Server {
    pub fn new(config: crate::config::ServerConfig) -> Self {
        Self {
            listeners: HashMap::new(),
            server_name: config.server_name,
            host: config.host,
            ports: config.ports,
            is_default: config.is_default,
            routes: config.routes.into_iter().map(Route::from).collect(),
            client_max_body_size: config.client_max_body_size,
        }
    }

    pub fn add_listener(&mut self, listener: EpollListener) -> io::Result<()> {
        let id = listener.epoll_fd;
        self.listeners.insert(id, listener);
        Ok(())
    }

    pub fn listen_and_serve(&mut self) -> io::Result<()> {
        // Take ownership of the listeners
        let listeners = std::mem::take(&mut self.listeners);
        let mut listeners: Vec<EpollListener> = listeners.into_values().collect();

        info!(
            "Starting server: [{}] at {}:{}",
            self.server_name.join("/"),
            self.host,
            self.ports[0]
        );

        let epoll_fd = unsafe { libc::epoll_create1(0) };
        if epoll_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Register all listeners to the global epoll instance
        for listener in &listeners {
            let mut event = libc::epoll_event {
                events: (libc::EPOLLIN | libc::EPOLLET) as u32,
                u64: listener.listener.as_raw_fd() as u64,
            };

            if unsafe {
                libc::epoll_ctl(
                    epoll_fd,
                    libc::EPOLL_CTL_ADD,
                    listener.listener.as_raw_fd(),
                    &mut event,
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
        }

        // Epoll event loop (single thread, handles all listeners)
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; MAX_EVENTS];

        loop {
            let nfds =
                unsafe { libc::epoll_wait(epoll_fd, events.as_mut_ptr(), MAX_EVENTS as i32, -1) };

            if nfds < 0 {
                return Err(io::Error::last_os_error());
            }

            for n in 0..nfds {
                let fd = events[n as usize].u64 as RawFd;

                // Find the corresponding listener
                if let Some(listener) = listeners.iter_mut().find(|l| l.listener.as_raw_fd() == fd)
                {
                    match listener.accept_connection() {
                        Ok(_) => (),
                        Err(e) => error!("Accept error: {}", e),
                    }
                } else {
                    // Find the connection and process data
                    for listener in listeners.iter_mut() {
                        if listener.connections.contains_key(&fd) {
                            match listener.handle_connection(fd) {
                                Ok(_) => (),
                                Err(e) => {
                                    warn!("Connection error: {}", e);
                                    let _ = listener.remove_connection(fd);
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}
