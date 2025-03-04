use std::io;

use kang::info;
use kang::server;

fn main() -> io::Result<()> {
    let mut server = server::Server::new("127.0.0.1:8080")?;

    info!("Kickstarting kang service");

    server.run()
}
