#[macro_use]
extern crate tracing;

use clap::{Parser, Subcommand, ValueEnum};
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
    Server {
        taget: SocketAddr,
        local_port: usize,
    },
    Client {
        target: SocketAddr,
        local_port: usize,
    },
    ListIp,
}

#[derive(Debug, Clone, ValueEnum)]
enum Mode {
    Ipv4,
    Ipv6,
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

    match args.command {
        Commands::Server { taget, local_port } => {
            let _ = Server::init(taget, local_port)?;
        }
        Commands::Client { target, local_port } => {
            let _ = Client::init(target, local_port)?;
        }
        Commands::ListIp => {
            let ips: (Vec<String>, Vec<String>) = Stun::resolve_public_address()?;
            println!("address: {:#?}", ips);
        }
    }

    Ok(())
}
