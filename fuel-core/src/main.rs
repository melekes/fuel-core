use service::FuelService;
use std::io;
use structopt::StructOpt;
use tracing::trace;

mod args;
pub(crate) mod chain_config;
pub mod database;
pub(crate) mod executor;
pub mod model;
pub mod schema;
pub mod service;
pub mod state;
pub(crate) mod tx_pool;

#[tokio::main]
async fn main() -> io::Result<()> {
    trace!("Initializing in TRACE mode.");
    // load configuration
    let config = args::Opt::from_args().exec()?;
    // initialize the server
    let server = FuelService::new_node(config).await?;
    // pause the main task while service is running
    server.run().await;
    Ok(())
}
