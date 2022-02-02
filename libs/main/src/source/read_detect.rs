use super::FsBlock;
use anyhow::{format_err, Result};
use block_iter_core::bitcoin::consensus::Decodable;
use block_iter_core::bitcoin::{Block, BlockHash, Network};
use fallible_iterator::FallibleIterator;
use fallible_iterator::{ IteratorExt};
use itertools::Itertools;
use log::{error, info};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Save half memory in comparison to using directly HashSet<BlockHash> while providing enough
/// bytes to reasonably prevent collisions. Use the non-zero part of the hash
struct Seen(HashSet<[u8; 12]>);
impl Seen {
    fn new() -> Seen {
        Seen(HashSet::new())
    }
    fn insert(&mut self, hash: &BlockHash) -> bool {
        let key: [u8; 12] = (&hash[..12]).try_into().unwrap();
        self.0.insert(key)
    }
}

pub struct DetectedBlock {
    start: usize,
    end: usize,
    hash: BlockHash,
    prev: BlockHash,
}

pub struct ReadDetect {
    iter: Box<dyn FallibleIterator<Item = FsBlock, Error = anyhow::Error> + Send>,
}
impl DetectedBlock {
    fn into_fs_block(self, file: &Arc<Mutex<File>>) -> FsBlock {
        FsBlock {
            start: self.start,
            end: self.end,
            hash: self.hash,
            prev: self.prev,
            file: Arc::clone(file),
            next: vec![],
        }
    }
}

impl ReadDetect {
    pub fn new(blocks_dir: &Path, network: Network) -> Result<Self> {
        let block_files_glob = blocks_dir.join("blk*.dat");
        info!("listing block files at {:?}", &block_files_glob);
        let mut paths: Vec<PathBuf> = glob::glob(
            block_files_glob
                .to_str()
                .ok_or_else(|| format_err!("Glob incorrect"))?,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format_err!("Path error: {}", e))?;
        paths.sort();
        info!("There are {} block files", paths.len());
        let mut seen = Seen::new();

        let iter = paths
            .into_iter()
            .map(move |path| {
                let file = File::open(&path)?;
                let mut reader = BufReader::new(file);
                let detected_blocks = detect(&mut reader, network.magic())?;
                drop(reader);

                let file = File::open(&path)?;
                let file = Arc::new(Mutex::new(file));

                let fs_blocks: Vec<_> = detected_blocks
                    .into_iter()
                    .filter(|e| seen.insert(&e.hash))
                    .map(|e| e.into_fs_block(&file))
                    .collect();

                // TODO if 0 blocks found, maybe wrong directory

                Ok(fs_blocks)
            })
            .flatten_ok()
            .transpose_into_fallible();

        Ok(Self {
            iter: Box::new(iter),
        })
    }
}

impl FallibleIterator for ReadDetect {
    type Item = FsBlock;
    type Error = anyhow::Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        self.iter.next()
    }
}

pub fn detect<R: Read + Seek>(mut reader: &mut R, magic: u32) -> Result<Vec<DetectedBlock>> {
    let mut rolling = RollingU32::default();

    // Instead of sending DetecetdBlock on the channel directly, we quickly insert in the vector
    // allowing to read ahead exactly one file (reading no block ahead cause non-parallelizing
    // reading, more than 1 file ahead cause cache to work not efficiently)
    let mut detected_blocks = Vec::with_capacity(128);

    loop {
        match u8::consensus_decode(&mut reader) {
            Ok(value) => {
                rolling.push(value);
                if magic != rolling.as_u32() {
                    continue;
                }
            }
            Err(_) => break, // EOF
        };
        let size = u32::consensus_decode(&mut reader)?;
        let start = reader.stream_position()? as usize;
        match Block::consensus_decode(&mut reader) {
            Ok(block) => {
                let end = reader.stream_position()? as usize;
                assert_eq!(size as usize, end - start);
                let hash = block.header.block_hash();
                let detected_block = DetectedBlock {
                    start,
                    end,
                    hash,
                    prev: block.header.prev_blockhash,
                };
                detected_blocks.push(detected_block);
            }
            Err(e) => {
                // It's mandatory to use stream_position (require MSRV 1.51) because I can't maintain
                // a byte read position because in case of error I don't know how many bytes of the
                // reader has been consumed
                error!("error block parsing {:?}", e)
            }
        }
    }
    Ok(detected_blocks)
}

/// Implements a rolling u32, every time a new u8 is `push`ed the old value is shifted by 1 byte
/// Allows to read a stream searching for a u32 magic without going back
#[derive(Default, Debug, Copy, Clone)]
struct RollingU32(u32);
impl RollingU32 {
    fn push(&mut self, byte: u8) {
        self.0 >>= 8;
        self.0 |= (byte as u32) << 24;
    }
    fn as_u32(&self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod test {
    use super::RollingU32;

    #[test]
    fn test_rolling() {
        let mut rolling = RollingU32::default();
        rolling.push(0x0B);
        assert_eq!(
            rolling.as_u32(),
            u32::from_be_bytes([0x0B, 0x00, 0x00, 0x00])
        );
        rolling.push(0x11);
        assert_eq!(
            rolling.as_u32(),
            u32::from_be_bytes([0x11, 0x0b, 0x00, 0x00])
        );
        rolling.push(0x09);
        assert_eq!(
            rolling.as_u32(),
            u32::from_be_bytes([0x09, 0x11, 0x0B, 0x00])
        );
        rolling.push(0x07);
        assert_eq!(
            rolling.as_u32(),
            u32::from_be_bytes([0x07, 0x09, 0x11, 0x0B])
        );
        assert_eq!(rolling.as_u32(), bitcoin::Network::Testnet.magic())
    }
}
