use std::io;
use std::process::ExitCode;

use post_server::{Server, port_from};

#[tokio::main]
async fn main() -> ExitCode {
    let port = match port_from(std::env::args().skip(1)) {
        Ok(port) => port,
        Err(usage) => {
            eprintln!("{usage}");
            return ExitCode::FAILURE;
        }
    };
    match run(port).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("post-server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(port: u16) -> io::Result<()> {
    let server = Server::bind(port).await?;
    for address in server.addresses() {
        println!(
            "Post server {} listens on {address}.",
            env!("CARGO_PKG_VERSION")
        );
    }
    println!("Press Ctrl+C to stop the server.");
    server
        .run(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
