use anyhow::{bail, Result};
use bitcoincore_rpc::RpcApi;
use block_iter_core::{bitcoin, BlockHash, BlockHeight};

mod fetcher;
pub use fetcher::Fetcher;

/// An minimum interface for node rpc for fetching blocks
pub trait Rpc: Send + Sync {
    type Data: Send;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64;
    const RECOMMENDED_ERROR_RETRY_DELAY_MS: u64;

    fn get_block_count(&self) -> Result<BlockHeight>;

    fn get_block_id_by_height(&self, height: BlockHeight) -> Result<Option<BlockHash>>;

    /// Get the block by id, along with id of the previous block
    fn get_block_by_id(&self, hash: &BlockHash) -> Result<Option<Self::Data>>;
}

impl Rpc for bitcoincore_rpc::Client {
    type Data = bitcoin::Block;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64 = 2000;
    const RECOMMENDED_ERROR_RETRY_DELAY_MS: u64 = 100;

    fn get_block_count(&self) -> Result<BlockHeight> {
        Ok(RpcApi::get_block_count(self)? as u32)
    }

    fn get_block_id_by_height(&self, height: BlockHeight) -> Result<Option<BlockHash>> {
        match self.get_block_hash(u64::from(height)) {
            Err(e) => {
                if e.to_string().contains("Block height out of range") {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
            Ok(o) => Ok(Some(o)),
        }
    }

    fn get_block_by_id(&self, hash: &BlockHash) -> Result<Option<Self::Data>> {
        let block: bitcoin::Block = match self.get_by_id(hash) {
            Err(e) => {
                if e.to_string().contains("Block height out of range") {
                    return Ok(None);
                } else {
                    return Err(e.into());
                }
            }
            Ok(o) => o,
        };

        Ok(Some(block))
    }
}

#[derive(Clone, Debug)]
pub struct RpcInfo {
    pub url: String,
    pub auth: bitcoincore_rpc::Auth,
}

impl RpcInfo {
    pub fn from_url(url_str: &str) -> Result<Self> {
        let mut url = url::Url::parse(url_str)?;
        let auth = match (url.username() == "", url.password()) {
            (false, Some(p)) => {
                bitcoincore_rpc::Auth::UserPass(url.username().to_owned(), p.to_owned())
            }
            (true, None) => bitcoincore_rpc::Auth::None,
            _ => bail!("Incorrect node auth parameters"),
        };
        url.set_password(None).expect("url lib sane");
        url.set_username("").expect("url lib sane");

        Ok(Self {
            url: url.to_string(),
            auth,
        })
    }
    pub fn to_rpc_client(&self) -> Result<bitcoincore_rpc::Client> {
        Ok(bitcoincore_rpc::Client::new(&self.url, self.auth.clone())?)
    }
}
