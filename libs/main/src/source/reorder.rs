use super::{block_extra::BlockExtra, FsBlock};
use anyhow::Result;
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::{BlockHash, Network};
use block_iter_core::BlockHeight;
use fallible_iterator::FallibleIterator;
use log::warn;
use std::collections::HashMap;
use std::convert::TryInto;

struct OutOfOrderBlocks {
    blocks: HashMap<BlockHash, FsBlock>,
    follows: HashMap<BlockHash, Vec<BlockHash>>,
    max_reorg: u8,
}

impl OutOfOrderBlocks {
    fn new(max_reorg: u8) -> Self {
        OutOfOrderBlocks {
            blocks: HashMap::default(),
            follows: HashMap::default(),
            max_reorg,
        }
    }

    fn add(&mut self, mut raw_block: FsBlock) {
        let prev_hash = raw_block.prev;
        self.follows
            .entry(prev_hash)
            .or_default()
            .push(raw_block.hash);

        if let Some(follows) = self.follows.remove(&raw_block.hash) {
            for el in follows {
                raw_block.next.push(el);
            }
        }

        if let Some(prev_block) = self.blocks.get_mut(&prev_hash) {
            prev_block.next.push(raw_block.hash);
        }

        self.blocks.insert(raw_block.hash, raw_block);
    }

    /// check the block identified by `hash` has at least `self.max_reorgs` blocks after, to be sure it's not a reorged block
    /// keep track of the followed `path` that should be initialized with empty vec in the first call
    fn exist_and_has_followers(&self, hash: &BlockHash, path: Vec<BlockHash>) -> Option<BlockHash> {
        if path.len() == self.max_reorg as usize {
            return Some(path[0]);
        }
        if let Some(block) = self.blocks.get(hash) {
            for next in block.next.iter() {
                let mut path = path.clone();
                path.push(*next);
                if let Some(hash) = self.exist_and_has_followers(next, path) {
                    return Some(hash);
                }
            }
        }
        None
    }

    fn remove(&mut self, hash: &BlockHash) -> Option<FsBlock> {
        if let Some(next) = self.exist_and_has_followers(hash, vec![]) {
            let mut value = self.blocks.remove(hash).unwrap();
            if value.next.len() > 1 {
                warn!("at {} fork to {:?} took {}", value.hash, value.next, next);
            }
            value.next = vec![next];
            Some(value)
        } else {
            None
        }
    }
}

pub struct Reorder<I> {
    iter: I,
    height: BlockHeight,
    next: BlockHash,
    blocks: OutOfOrderBlocks,
}

impl<I> Reorder<I>
where
    I: FallibleIterator<Item = FsBlock>,
{
    pub fn new(network: Network, max_reorg: u8, iter: I) -> Self {
        Self {
            height: 0,
            next: genesis_block(network).block_hash(),
            blocks: OutOfOrderBlocks::new(max_reorg),
            iter,
        }
    }
}

impl<I> FallibleIterator for Reorder<I>
where
    I: FallibleIterator<Item = FsBlock, Error = anyhow::Error>,
{
    type Item = BlockExtra;
    type Error = anyhow::Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        loop {
            if let Some(stored_block) = self.blocks.remove(&self.next) {
                let mut block_extra: BlockExtra = stored_block.try_into()?;
                self.next = block_extra.next[0];
                block_extra.height = self.height;
                self.blocks.follows.remove(&block_extra.block_hash);
                self.blocks
                    .blocks
                    .remove(&block_extra.block.header.prev_blockhash);
                self.height += 1;
                return Ok(Some(block_extra));
            }

            match self.iter.next() {
                Ok(Some(raw_block)) => {
                    // even tough should be 1024 -> https://github.com/bitcoin/bitcoin/search?q=BLOCK_DOWNLOAD_WINDOW
                    // in practice it needs to be greater
                    let max_block_to_reorder = 10_000;
                    if self.blocks.blocks.len() > max_block_to_reorder {
                        for block in self.blocks.blocks.values() {
                            println!("{} {:?}", block.hash, block.next);
                        }
                        println!("next: {}", self.next);
                        panic!("Reorder map grow more than {}", max_block_to_reorder);
                    }
                    self.blocks.add(raw_block);
                }
                Err(e) => return Err(e.into()),
                Ok(None) => {
                    return Ok(None);
                }
            }
        }
    }
}
