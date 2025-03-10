use std::os::fd::RawFd;
use std::os::unix::io::AsRawFd;
use std::{collections::HashMap, io};

use crate::server::epoll::MAX_EVENTS;
use crate::server::route::Route;
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
            "Serving: [{}] at {}:{}",
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
                let event = &events[n as usize];
                let fd = event.u64 as RawFd;
                let events = event.events;

                // Find the corresponding listener
                if let Some(listener) = listeners.iter_mut().find(|l| l.listener.as_raw_fd() == fd)
                {
                    info!("Accepting New Connections");
                    // New connection available
                    if events & libc::EPOLLIN as u32 != 0 {
                        match listener.accept_connection(epoll_fd) {
                            Ok(_) => (),
                            Err(e) => error!("Accept error: {}", e),
                        }
                    }
                } else {
                    info!("Handling Existing Connections");
                    // Handle existing connection
                    for listener in listeners.iter_mut() {
                        if listener.connections.contains_key(&fd) {
                            // Only process if we have read events
                            if events & libc::EPOLLIN as u32 != 0 {
                                match listener.handle_connection(fd) {
                                    Ok(req) => {
                                        // Find matching route
                                        let route = self
                                            .routes
                                            .iter()
                                            .find(|r| r.path == req.path())
                                            .unwrap_or_else(|| &self.routes[0]);

                                        info!("Handling route: {}", route.path);

                                        // Process request and send response
                                        let response = route.handle();
                                        match listener.send_bytes(response.to_bytes(), fd) {
                                            Ok(_) => {
                                                info!("Response sent successfully to fd={}", fd);
                                                let _ = listener.remove_connection(fd, epoll_fd);
                                            }
                                            Err(e) => {
                                                error!("Failed to send response: {}", e);
                                                let _ = listener.remove_connection(fd, epoll_fd);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        match e.kind() {
                                            io::ErrorKind::WouldBlock => {
                                                // Not enough data yet, keep connection open
                                                info!("Waiting for more data on fd={}", fd);
                                            }
                                            _ => {
                                                warn!("Connection error: {}", e);
                                                let _ = listener.remove_connection(fd, epoll_fd);
                                            }
                                        }
                                    }
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
