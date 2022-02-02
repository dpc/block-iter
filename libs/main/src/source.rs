use block_iter_core::BlockHash;
use std::{
    fs::File,
    sync::{Arc, Mutex},
};

pub mod block_extra;
pub mod read_detect;
pub mod reorder;

/// Before reorder we keep only the position of the block in the file system and data relative
/// to the block hash, the previous hash and the following hash (populated during reorder phase)
/// We will need to read the block from disk again, but by doing so we will avoid using too much
/// memory in the [`OutOfOrderBlocks`] map.
#[derive(Debug)]
pub struct FsBlock {
    /// the file the block identified by `hash` is stored in. Multiple blocks are stored in the
    /// and we don't want to open/close the file many times for performance reasons so it's shared.
    /// It's a Mutex to allow to be sent between threads but only one thread (reorder) mutably
    /// access to it so there is no contention. (Arc alone isn't enough cause it can't be mutated,
    /// RefCell can be mutated but not sent between threads)
    pub file: Arc<Mutex<File>>,

    /// The start position in bytes in the `file` at which the block identified by `hash`
    pub start: usize,

    /// The end position in bytes in the `file` at which the block identified by `hash`
    pub end: usize,

    /// The hash identifying this block, output of `block.header.block_hash()`
    pub hash: BlockHash,

    /// The hash of the block previous to this one, `block.header.prev_blockhash`
    pub prev: BlockHash,

    /// The hash of the blocks following this one. It is populated during the reorder phase, it can
    /// be more than one because of reorgs.
    pub next: Vec<BlockHash>,
}
