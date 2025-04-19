use crate::{
    config::{Config, ErrorPages, ServerConfig}, error, http::SessionStore, info, server::{Listener, Mux, MAX_EVENTS}, warn
};

use std::os::fd::RawFd;
use std::{collections::HashMap, io};

#[cfg(target_os = "linux")]
use libc::{
    epoll_create1, epoll_ctl, epoll_event, epoll_wait, EPOLLET, EPOLLIN, EPOLLOUT, EPOLL_CTL_ADD,
};

#[cfg(target_os = "macos")]
use libc::{kevent, kqueue, EVFILT_READ, EV_ADD, EV_ENABLE};

pub struct Server {
    pub listeners: HashMap<i32, Box<dyn Listener>>,
    pub server_name: Vec<String>,
    pub host: String,
    pub ports: Vec<u16>,
    pub is_default: bool,
    pub mux: Mux,
    pub client_max_body_size: Option<String>,
    pub error_pages: ErrorPages,
    pub session_store: Option<SessionStore>,
}

impl Server {
    pub fn new(server_config: ServerConfig, config: Config) -> Server {
        // Clone server_config before using it to avoid partial move issues
        let server_config_clone = server_config.clone();

        // Initialize session store if sessions are enabled
        let session_store = if server_config.sessions.enabled {
            Some(SessionStore::new(server_config.sessions.timeout_minutes))
        } else if config.global.sessions.enabled {
            Some(SessionStore::new(config.global.sessions.timeout_minutes))
        } else {
            None
        };

        Server {
            listeners: HashMap::new(),
            server_name: server_config.server_name,
            host: server_config.host,
            ports: server_config.ports,
            is_default: server_config.is_default,
            mux: Mux::new(server_config_clone, config),
            client_max_body_size: server_config.client_max_body_size,
            error_pages: server_config.error_pages,
            session_store,
        }
    }

    pub fn add_listener<T: Listener + 'static + Send + Sync>(
        &mut self,
        listener: T,
    ) -> io::Result<()> {
        let id = listener.get_id();
        self.listeners.insert(id, Box::new(listener));
        Ok(())
    }

    pub fn listen_and_serve(&mut self) -> io::Result<()> {
        // Take ownership of the listeners
        let listeners = std::mem::take(&mut self.listeners);
        let mut listeners: Vec<Box<dyn Listener>> = listeners.into_values().collect();

        info!(
            "Serving: [{}] at {}:{}",
            self.server_name.join("/"),
            self.host,
            self.ports[0]
        );

        #[cfg(target_os = "linux")]
        let global_fd = unsafe { epoll_create1(0) };
        #[cfg(target_os = "macos")]
        let global_fd = unsafe { kqueue() };

        if global_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Register all listeners to the global instance
        for listener in &listeners {
            #[cfg(target_os = "linux")]
            {
                let mut event = epoll_event {
                    events: (EPOLLIN | EPOLLET) as u32,
                    u64: listener.get_id() as u64,
                };

                if unsafe { epoll_ctl(global_fd, EPOLL_CTL_ADD, listener.get_id(), &mut event) } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }

            #[cfg(target_os = "macos")]
            {
                let changes = kevent {
                    ident: listener.get_id() as usize,
                    filter: EVFILT_READ as i16,
                    flags: EV_ADD | EV_ENABLE,
                    fflags: 0,
                    data: 0,
                    udata: std::ptr::null_mut(),
                };

                if unsafe {
                    kevent(
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
        let mut events = vec![epoll_event { events: 0, u64: 0 }; MAX_EVENTS];
        #[cfg(target_os = "macos")]
        let mut events = vec![
            kevent {
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
            let nfds = unsafe { epoll_wait(global_fd, events.as_mut_ptr(), MAX_EVENTS as i32, -1) };

            #[cfg(target_os = "macos")]
            let nfds = unsafe {
                kevent(
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
                let (fd, event_filter, event_data) = {
                    let event = &events[n as usize];
                    (event.ident as RawFd, event.filter, event.data)
                };

                // First check if this is a listener socket
                if let Some(listener) = listeners.iter_mut().find(|l| l.get_id() == fd) {
                    #[cfg(target_os = "linux")]
                    let has_read_event = events & EPOLLIN as u32 != 0;
                    #[cfg(target_os = "macos")]
                    let has_read_event = event_filter == EVFILT_READ as i16;

                    if has_read_event {
                        match listener.accept_connection(global_fd) {
                            Ok(_) => (),
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // No more connections to accept
                                continue;
                            }
                            Err(e) => error!("Accept error: {}", e),
                        }
                    }
                } else {
                    // This is a connected socket
                    let mut handled: bool = false;
                    for listener in listeners.iter_mut() {
                        #[cfg(target_os = "linux")]
                        let has_read_event = events & EPOLLIN as u32 != 0;
                        #[cfg(target_os = "macos")]
                        let has_read_event = event_filter == EVFILT_READ as i16 && event_data > 0;

                        if has_read_event {
                            match listener.handle_connection(fd) {
                                Ok(req) => {
                                    info!(
                                        "Parsed HTTP Request:
{:#?}",
                                        req
                                    );
                                    //let res = self.mux.handle(req);
                                    let mut res = if let Some(session_store) = &mut self.session_store {
                                        if rand::random::<f32>() < 0.01 {
                                            session_store.cleanup_expired();
                                        }

                                        // First, get the session ID from the request
                                        let session_id = req.headers()
                                            .get_cookie("session_id")
                                            .map(|c| c.value.clone());

                                        // First, handle the session and extract just the session ID string
                                        let session_id_for_cookie = {
                                            // Get or create the session
                                            let session = if let Some(id) = &session_id {
                                                // Try to get existing session
                                                if let Some(existing_session) = session_store.get_session(id) {
                                                    existing_session
                                                } else {
                                                    // Create new session if not found
                                                    session_store.create_session()
                                                }
                                            } else {
                                                // No session ID provided, create new session
                                                session_store.create_session()
                                            };
                                            // Extract just the session ID as a string
                                            session.id.clone()
                                        }; // End of mutable borrow scope

                                        let mut resp = self.mux.handle(req);

                                        let cookie = session_store.create_session_cookie(&session_id_for_cookie);
                                        resp.add_cookie(cookie);

                                        resp
                                    } else {
                                        self.mux.handle(req)
                                    };

                                    match listener.send_bytes(res.to_bytes(), fd) {
                                        Ok(_) => {
                                            handled = true;
                                            let _ = listener.remove_connection(fd, global_fd);
                                            break;
                                        }
                                        Err(e) => {
                                            error!("Failed to send response: {}", e);
                                            handled = true;
                                            let _ = listener.remove_connection(fd, global_fd);
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to handle connection: {}", e);
                                    match e.kind() {
                                        io::ErrorKind::WouldBlock => {
                                            // Not enough data yet, keep connection open
                                            handled = true;
                                            break;
                                        }
                                        _ => {
                                            warn!("Connection error: {}", e);
                                            handled = true;
                                            let _ = listener.remove_connection(fd, global_fd);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // If no listener handled this fd, remove it from all listeners
                    if !handled {
                        for listener in listeners.iter_mut() {
                            let _ = listener.remove_connection(fd, global_fd);
                        }
                    }
                }
            }
        }
    }
}
