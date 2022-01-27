pub use bitcoin::hash_types::{BlockHash, Txid};
pub use bitcoin::hashes::{hash160::Hash as Hash160, sha256d::Hash as Sha256dHash};
pub use bitcoin::hashes::{hex::FromHex as _, Hash};

pub trait WithBlockHash {
    fn block_hash(&self) -> &BlockHash;
}

pub trait WithPrevBlockHash {
    fn prev_block_hash(&self) -> &BlockHash;
}

pub trait WithBlockHeight {
    fn block_height(&self) -> BlockHeight;
}

pub trait WithTransactions {
    fn transactions(&self) -> &[bitcoin::blockdata::transaction::Transaction];
}

impl WithTransactions for bitcoin::Block {
    fn transactions(&self) -> &[bitcoin::blockdata::transaction::Transaction] {
        &self.txdata
    }
}

impl<D> WithTransactions for WithHeightAndId<D>
where
    D: WithTransactions,
{
    fn transactions(&self) -> &[bitcoin::blockdata::transaction::Transaction] {
        &self.data.transactions()
    }
}

// not implemented, because hash is calculated by encoding the struct,
// so it's too slow
// impl WithBlockHash for bitcoin::Block {}

impl WithPrevBlockHash for bitcoin::Block {
    fn prev_block_hash(&self) -> &BlockHash {
        &self.header.prev_blockhash
    }
}
/// Data in a block
///
/// Comes associated with height and hash of the block.
///
/// `T` is type type of the data.
pub struct WithHeightAndId<D> {
    pub height: BlockHeight,
    pub id: BlockHash,
    pub data: D,
}

pub struct WithId<H, D = ()> {
    pub id: H,
    pub data: D,
}

pub type WithHash<T> = WithId<Sha256dHash, T>;
pub type WithTxId<T> = WithId<Txid, T>;

pub type BlockHeight = u32;

pub struct BlockHeightAndHash {
    pub height: BlockHeight,
    pub hash: BlockHash,
}

/// Block data from BitcoinCore (`rust-bitcoin`)
pub type BlockData = WithHeightAndId<bitcoin::Block>;

pub type BlockHex = String;
pub type TxHex = String;
pub type TxHash = Sha256dHash;
