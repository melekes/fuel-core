use ethers_core::*;

use ethers_providers::{Http, Middleware, Provider, StreamExt};
use std::convert::TryFrom;

use std::env;
use env_logger::Builder;

#[tokio::main]
async fn main() {
    Builder::new().parse_env(&env::var("MY_APP_LOG").unwrap_or_default()).init();

    let provider =
        Provider::<Http>::try_from("https://mainnet.infura.io/v3/c60b0bb42f8a4c6481ecd229eddaca27")
            .expect("could not instantiate HTTP Provider");
    /*
        From infure: newBlockFilter
        Creates a filter in the node, to notify when a new block arrives. To check if the state has changed, call eth_getFilterChanges.
        Filter IDs will be valid for up to fifteen minutes, and can polled by any connection using the same v3 project ID.
    */
    // it seems thaat provider periodically featched data every 7s (maybe high for us)
    /* seems good intro: https://tms-dev-blog.com/rust-web3-connect-to-ethereum/ */
    /* examples: https://github.com/gakonst/ethers-rs/tree/master/examples */
    /* it seems ethers was inspired by web3 here are examples that can be useful: https://github.com/tomusdrw/rust-web3/tree/master/examples */

    let block_watcher = provider.watch_blocks().await.expect("all good");
    let mut stream = block_watcher.stream();
    while let Some(block_hash) = stream.next().await {
        println!("received block: {:?}", block_hash);
        let block = provider.get_block(block_hash).await.expect("BLOCK");
        println!("Block received:{:?}",block);
    }

    let block = provider.get_block(100u64).await.expect("be good");
    println!(
        "Got block: {}",
        serde_json::to_string(&block).expect("be string")
    );
}
