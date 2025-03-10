use std::{env, thread};

use kang::config::Config;
use kang::server::EpollListener;
use kang::{error, info, utils};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "config/kangrc".to_string());

    utils::draw_ascii();
    info!("Booting Kang Server");

    let config = Config::from_file(config_path)?;
    let mut servers = config.create_servers();

    // Create listeners for each server's ports
    for server in &mut servers {
        // Collect all the necessary information first
        let addrs: Vec<String> = server
            .ports
            .iter()
            .map(|port| format!("{host}:{port}", host = server.host, port = port))
            .collect();

        // Then create and add listeners
        for addr in addrs {
            let listener = EpollListener::new(&addr)?;
            server.add_listener(listener)?;
        }
    }

    // Start all servers in their own threads
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
