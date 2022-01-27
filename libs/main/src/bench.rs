use std::time::{Duration, Instant};

use block_iter_core::WithTransactions;

pub trait IteratorExt {
    fn bench_txs(self)
    where
        Self: Iterator,
        Self::Item: WithTransactions;
    fn bench_items(self);
}

impl<I> IteratorExt for I
where
    I: Iterator,
{
    fn bench_txs(mut self)
    where
        I::Item: WithTransactions,
    {
        let start = Instant::now();
        let mut last = Instant::now();
        let mut blocks = 0;
        let mut txs = 0;
        let mut blocks_total = 0;
        let mut txs_total = 0;
        while let Some(item) = self.next() {
            blocks += 1;
            blocks_total += 1;
            txs += item.transactions().len() as u64;
            txs_total += item.transactions().len() as u64;

            let now = Instant::now();
            let period = Duration::from_secs(1);
            let current_duration = now.duration_since(last);

            if current_duration >= period {
                let total_duration = now.duration_since(start);
                eprintln!(
                    "Current: {:>5} blk/s; {:>6} txs/s; Total: {:>5} blk/s; {:>6} tx/s; {:>8} blks; {:>6} txs",
                    blocks / current_duration.as_secs(),
                    txs / current_duration.as_secs(),
                    blocks_total / total_duration.as_secs(),
                    txs_total / total_duration.as_secs(),
                    blocks_total,
                    txs_total,
                );

                last = now;
                txs = 0;
                blocks = 0;
            }
        }
    }

    fn bench_items(mut self) {
        let start = Instant::now();
        let mut last = Instant::now();
        let mut blocks = 0;
        let mut blocks_total = 0;
        while let Some(_item) = self.next() {
            blocks += 1;
            blocks_total += 1;

            let now = Instant::now();
            let period = Duration::from_secs(1);
            let current_duration = now.duration_since(last);

            if current_duration >= period {
                let total_duration = now.duration_since(start);
                eprintln!(
                    "Current: {:>6} item/s; Total: {:>8} item/s; {:>10} items",
                    blocks / current_duration.as_secs(),
                    blocks_total / total_duration.as_secs(),
                    blocks_total,
                );

                last = now;
                blocks = 0;
            }
        }
    }
}
