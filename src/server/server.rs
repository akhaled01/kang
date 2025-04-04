use std::os::fd::RawFd;
use std::os::unix::io::AsRawFd;
use std::{collections::HashMap, io};

use crate::config::config::{Config, ServerConfig};
#[cfg(target_os = "linux")]
use crate::server::epoll::MAX_EVENTS;
#[cfg(target_os = "macos")]
use crate::server::kqueue::MAX_EVENTS;
use crate::server::route::Route;
use crate::{error, info, warn};

#[cfg(target_os = "linux")]
use libc::{
    epoll_create1, epoll_ctl, epoll_event, epoll_wait, EPOLLET, EPOLLIN, EPOLLOUT, EPOLL_CTL_ADD,
};

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
    pub fn new(server_config: ServerConfig, config: Config) -> Self {
        Self {
            listeners: HashMap::new(),
            server_name: server_config.server_name,
            host: server_config.host,
            ports: server_config.ports,
            is_default: server_config.is_default,
            routes: server_config
                .routes
                .into_iter()
                .map(|r| Route::from((r, config.clone())))
                .collect(),
            client_max_body_size: server_config.client_max_body_size,
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

        #[cfg(target_os = "linux")]
        let global_fd = unsafe { libc::epoll_create1(0) };
        #[cfg(target_os = "macos")]
        let global_fd = unsafe { libc::kqueue() };

        if global_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Register all listeners to the global instance
        for listener in &listeners {
            #[cfg(target_os = "linux")]
            {
                let mut event = libc::epoll_event {
                    events: (libc::EPOLLIN | libc::EPOLLET) as u32,
                    u64: listener.listener.as_raw_fd() as u64,
                };

                if unsafe {
                    libc::epoll_ctl(
                        global_fd,
                        libc::EPOLL_CTL_ADD,
                        listener.listener.as_raw_fd(),
                        &mut event,
                    )
                } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }

            #[cfg(target_os = "macos")]
            {
                let changes = libc::kevent {
                    ident: listener.listener.as_raw_fd() as usize,
                    filter: libc::EVFILT_READ as i16,
                    flags: libc::EV_ADD | libc::EV_ENABLE,
                    fflags: 0,
                    data: 0,
                    udata: std::ptr::null_mut(),
                };

                if unsafe {
                    libc::kevent(
                        global_fd,
                        &changes,
                        1,
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null(),
                    )
                } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
        }

        // Event loop (single thread, handles all listeners)
        #[cfg(target_os = "linux")]
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; MAX_EVENTS];
        #[cfg(target_os = "macos")]
        let mut events = vec![
            libc::kevent {
                ident: 0,
                filter: 0,
                flags: 0,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            MAX_EVENTS
        ];

        loop {
            #[cfg(target_os = "linux")]
            let nfds =
                unsafe { libc::epoll_wait(global_fd, events.as_mut_ptr(), MAX_EVENTS as i32, -1) };

            #[cfg(target_os = "macos")]
            let nfds = unsafe {
                libc::kevent(
                    global_fd,
                    std::ptr::null(),
                    0,
                    events.as_mut_ptr(),
                    MAX_EVENTS as i32,
                    std::ptr::null(),
                )
            };

            if nfds < 0 {
                return Err(io::Error::last_os_error());
            }

            for n in 0..nfds {
                #[cfg(target_os = "linux")]
                let (fd, events) = {
                    let event = &events[n as usize];
                    (event.u64 as RawFd, event.events)
                };

                #[cfg(target_os = "macos")]
                let (fd, events) = {
                    let event = &events[n as usize];
                    (
                        event.ident as RawFd,
                        if event.filter == libc::EVFILT_READ as i16 {
                            1
                        } else {
                            0
                        },
                    )
                };

                // Find the corresponding listener
                if let Some(listener) = listeners.iter_mut().find(|l| l.listener.as_raw_fd() == fd)
                {
                    info!("Accepting New Connections");
                    // New connection available
                    #[cfg(target_os = "linux")]
                    let has_read_event = events & libc::EPOLLIN as u32 != 0;
                    #[cfg(target_os = "macos")]
                    let has_read_event = events != 0;

                    if has_read_event {
                        match listener.accept_connection(global_fd) {
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
                            #[cfg(target_os = "linux")]
                            let has_read_event = events & libc::EPOLLIN as u32 != 0;
                            #[cfg(target_os = "macos")]
                            let has_read_event = events != 0;

                            if has_read_event {
                                match listener.handle_connection(fd) {
                                    Ok(req) => {
                                        // Find matching route by checking path prefixes
                                        let route = self
                                            .routes
                                            .iter()
                                            .filter(|r| req.path().starts_with(&r.path))
                                            .max_by_key(|r| r.path.len())
                                            .unwrap_or_else(|| &self.routes[0]);

                                        info!("Handling route: {}", route.path);

                                        // Process request and send response
                                        let response = route.handle(req);
                                        match listener.send_bytes(response.to_bytes(), fd) {
                                            Ok(_) => {
                                                info!("Response sent successfully to fd={}", fd);
                                            }
                                            Err(e) => {
                                                error!("Failed to send response: {}", e);
                                            }
                                        }
                                        let _ = listener.remove_connection(fd, global_fd);
                                    }
                                    Err(e) => {
                                        match e.kind() {
                                            io::ErrorKind::WouldBlock => {
                                                // Not enough data yet, keep connection open
                                                info!("Waiting for more data on fd={}", fd);
                                            }
                                            _ => {
                                                warn!("Connection error: {}", e);
                                                let _ = listener.remove_connection(fd, global_fd);
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
