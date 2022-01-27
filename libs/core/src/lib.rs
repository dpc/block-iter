mod types;

/// Re-export `bitcoin` so donwstream can stay in sync
pub use bitcoin;

pub use types::*;
pub type OwnedBlockData = Box<dyn Iterator<Item = types::BlockData>>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
