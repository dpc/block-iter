use anyhow::Result;
use block_iter::bench::IteratorExt as _;
use block_iter::rpc::{self, Fetcher};
use clap::Parser;
use std::sync::Arc;

#[derive(Debug, Parser, Clone)]
pub struct Opts {
    #[clap(env = "BITCOIN_CORE_RPC_URL")]
    bitcoin_core_rpc_url: String,
}

fn main() -> Result<()> {
    env_logger::init();
    let opts: Opts = clap::Parser::parse();

    let rpc_info = rpc::RpcInfo::from_url(&opts.bitcoin_core_rpc_url)?;
    let rpc = rpc_info.to_rpc_client()?;
    let rpc = Arc::new(rpc);

    let fetcher = Fetcher::new(rpc, None)?;

    fetcher.bench_txs();

    Ok(())
}
