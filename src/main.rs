use std::env;

use kang::config::Config;
use kang::info;
use kang::server::EpollListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "config/kangrc".to_string());

    let config = Config::from_file(config_path)?;
    let mut servers = config.create_servers();

    info!("Kickstarting kang service");

    // Create listeners for each server's ports
    for server in &mut servers {
        // Collect all the necessary information first
        let addrs: Vec<String> = server.ports
            .iter()
            .map(|port| format!("{host}:{port}", host = server.host, port = port))
            .collect();
            
        // Then create and add listeners
        for addr in addrs {
            let listener = EpollListener::new(&addr)?;
            server.add_listener(listener)?;
        }
    }

    // Start all servers
    for mut server in servers {
        server.listen_and_serve()?;
    }

    Ok(())
}
