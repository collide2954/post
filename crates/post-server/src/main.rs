use std::error::Error;
use std::process::ExitCode;

use post_server::{Command, Server, Source, USAGE, command_from, new_owner_link};

#[tokio::main]
async fn main() -> ExitCode {
    let command = match command_from(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(usage) => {
            eprintln!("{usage}");
            return ExitCode::FAILURE;
        }
    };
    let result = match command {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            println!("post-server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Run(source) => run(&source).await,
        Command::OwnerLink(source) => owner_link(&source),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("post-server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(source: &Source) -> Result<(), Box<dyn Error>> {
    let config = source.config(&std::env::current_dir()?)?;
    let server = Server::bind(&config).await?;
    for address in server.addresses() {
        println!(
            "Post server {} listens on {address}.",
            env!("CARGO_PKG_VERSION")
        );
    }
    println!("The data of the server is in {}.", config.data.display());
    if let Some(fingerprint) = server.fingerprint() {
        println!("The certificate of this server has this SHA-256 fingerprint:");
        println!("{}", fingerprint.grouped());
    }
    if let Some(link) = server.owner_link() {
        println!("This server has no owner. Paste this link into Post to become the owner:");
        println!("{link}");
    }
    println!("Press Ctrl+C to stop the server.");
    server
        .run(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

fn owner_link(source: &Source) -> Result<(), Box<dyn Error>> {
    let config = source.config(&std::env::current_dir()?)?;
    let link = new_owner_link(&config)?;
    println!("The person who pastes this link into Post becomes the server owner:");
    println!("{link}");
    println!("Start the server again. The link works when the server is in operation.");
    Ok(())
}
