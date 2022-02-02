use std::path::PathBuf;

use anyhow::Result;
use block_iter::{
    bench::FallibleIteratorExt as _,
    source::{read_detect::ReadDetect, reorder::Reorder},
};
use clap::Parser;
use dpc_pariter::IteratorExt as _;
use fallible_iterator::{FallibleIterator, IteratorExt as _};

#[derive(Debug, Parser, Clone)]
pub struct Opts {
    #[clap(env = "BITCOIN_CORE_BLOCKS_DIR", parse(from_os_str))]
    bitcoin_core_blocks_dir: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let opts: Opts = clap::Parser::parse();
    let network = bitcoin::Network::Bitcoin;

    Reorder::new(
        network,
        5,
        ReadDetect::new(&opts.bitcoin_core_blocks_dir, network)?
            // TODO: add support to `dpc-pariter`
            .iterator()
            .readahead(0)
            .transpose_into_fallible(),
    )
    .bench_txs()?;

    Ok(())
}
