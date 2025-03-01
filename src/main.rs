#[macro_use]
extern crate tracing;

use error::Result;
use stun::Stun;

mod error;
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

fn main() -> Result<()> {
    init_log();

    let gg = Stun::resolve_public_address()?;

    println!("address: {:#?}", gg);

    Ok(())
}
