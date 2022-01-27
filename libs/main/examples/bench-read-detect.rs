use std::path::PathBuf;

use anyhow::Result;
use block_iter::{bench::IteratorExt as _, source::read_detect::ReadDetect};
use clap::Parser;

#[derive(Debug, Parser, Clone)]
pub struct Opts {
    #[clap(env = "BITCOIN_CORE_BLOCKS_DIR", parse(from_os_str))]
    bitcoin_core_blocks_dir: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let opts: Opts = clap::Parser::parse();

    ReadDetect::new(&opts.bitcoin_core_blocks_dir, bitcoin::Network::Bitcoin)?.bench_items();

    Ok(())
}
