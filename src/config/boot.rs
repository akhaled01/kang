use std::{net::TcpListener, thread};

use super::config::Config;
use crate::{error, warn};
#[cfg(target_os = "macos")]
use crate::server::KqueueListener;
#[cfg(target_os = "linux")]
use crate::server::EpollListener;

pub struct KangStarter;

impl KangStarter {
    fn is_port_available(port: u16) -> bool {
        TcpListener::bind(format!("127.0.0.1:{}", port))
            .or_else(|_| TcpListener::bind(format!("[::1]:{}", port)))
            .is_ok()
    }

    pub fn boot_servers(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::from_file(config_path)?;
        let mut servers = config.create_servers();

        // Create listeners for each server's ports
        for server in &mut servers {
            for &original_port in &server.ports.clone() {
                let mut current_port = original_port;
                let max_attempts = 100; // Try up to 100 ports

                // Try to find an available port
                for attempt in 0..max_attempts {
                    // Check if port is available before trying to bind
                    if !Self::is_port_available(current_port) {
                        warn!("Port {current_port} is in use, trying next port...");
                        current_port += 1;
                        continue;
                    }

                    let addr = format!("{host}:{port}", host = server.host, port = current_port);
                    #[cfg(target_os = "macos")]
                    let listener_result = KqueueListener::new(&addr);
                    #[cfg(target_os = "linux")]
                    let listener_result = EpollListener::new(&addr);

                    match listener_result {
                        Ok(listener) => {
                            if current_port != original_port {
                                warn!("Port {original_port} was in use, using port {current_port} instead");
                            }
                            server.add_listener(listener)?;
                            break;
                        }
                        Err(_) if attempt < max_attempts - 1 => {
                            current_port += 1;
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
            }
        }

        let mut handles = Vec::new();

        for mut server in servers {
            let handle = thread::spawn(move || {
                if let Err(e) = server.listen_and_serve() {
                    error!("Server error: {}", e);
                }
            });
            handles.push(handle);
        }

        // Wait for all server threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        Ok(())
    }
}
