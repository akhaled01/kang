use std::io;

use kang::info;
use kang::server;

fn main() -> io::Result<()> {
    info!("Current working directory: {:?}", std::env::current_dir()?);
    let mut server = server::Server::new("127.0.0.1:8080")?;

    info!("Kickstarting kang service");
    info!("Server running at http://127.0.0.1:8080");
    info!("Upload test page available at http://127.0.0.1:8080/upload.html");

    server.run()
}
