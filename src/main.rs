#[macro_use]
extern crate tracing;

use clap::{Parser, Subcommand, ValueEnum};
use error::Result;
use listner::Listner;
use std::net::SocketAddr;
use stun::Stun;

mod client;
mod constant;
mod error;
mod listner;
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
    Server { mode: Mode },
    Client { target: SocketAddr },
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
        Commands::Server { mode } => {
            println!("mode: {:#?}", mode);
        }
        Commands::Client { target } => {
            let _ = Listner::init(target, 3000)?;
        }
        Commands::ListIp => {
            let ips: (Vec<String>, Vec<String>) = Stun::resolve_public_address()?;
            println!("address: {:#?}", ips);
        }
    }

    Ok(())
}
