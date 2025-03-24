use kang::{config::boot::KangStarter, info, utils};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "config/kangrc".to_string());

    utils::draw_ascii();
    info!("Booting Kang Server");

    KangStarter::boot_servers(&config_path)?;

    Ok(())
}
