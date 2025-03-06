use std::{collections::HashMap, io};

use crate::error;

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
        use std::thread;
        let mut handles = Vec::new();

        // Take ownership of the listeners
        let listeners = std::mem::take(&mut self.listeners);

        // Convert HashMap into Vec of listeners to avoid borrowing issues
        let listeners: Vec<_> = listeners.into_values().collect();

        // Spawn threads for each listener
        for mut listener in listeners {
            let handle = thread::spawn(move || {
                if let Err(e) = listener.run() {
                    error!("Error serving listener: {}", e);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to finish
        for handle in handles {
            if let Err(e) = handle.join() {
                eprintln!("Thread join error: {:?}", e);
            }
        }

        Ok(())
    }
}
