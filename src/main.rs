use std::io;

use kang::info;
use kang::server;

fn main() -> io::Result<()> {
    let mut server = server::EpollListener::new("127.0.0.1:8080")?;

    info!("Kickstarting kang service");
    info!("Server running at http://127.0.0.1:8080");
    info!("Upload test page available at http://127.0.0.1:8080/upload.html");

    server.run()
}
