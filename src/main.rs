#[macro_use]
extern crate tracing;

use clap::{Parser, Subcommand};
use client::Client;
use error::Result;
use server::Server;
use std::net::SocketAddr;
use stun::Stun;

mod client;
mod constant;
mod error;
mod server;
mod stun;

fn init_log() {
    use std::env;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "pingo=debug");
    }

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}

#[derive(Debug, Clone, Subcommand)]
enum Commands {
    Server { taget: SocketAddr },
    Client { target: SocketAddr },
    ListIp,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    init_log();

    let args = Args::parse();

    debug!("args passed: {:#?}", args);

    let addrs = Stun::resolve_public_address()?;

    let ipv4_addr = addrs
        .iter()
        .find(|addr| addr.public_address.is_ipv4())
        .unwrap();

    info!("public addresses: {:#?}", addrs);

    match args.command {
        Commands::Server { taget } => {
            let _ = Server::init(taget, ipv4_addr.local_port.clone())?;
        }
        Commands::Client { target } => {
            let _ = Client::init(target, ipv4_addr.local_port.clone())?;
        }
        Commands::ListIp => {}
    }

    Ok(())
}
