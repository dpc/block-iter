use std::time::{Duration, Instant};

use block_iter_core::WithTransactions;
use fallible_iterator::{FallibleIterator, IteratorExt as _};

pub trait IteratorExt {
    fn bench_txs(self)
    where
        Self: Iterator,
        Self: Sized,
        Self::Item: WithTransactions,
    {
        self.into_fallible()
            .bench_txs()
            .map_err(|_e| ())
            .expect("infallible iterator somehow returned an error");
    }

    fn bench_items(self)
    where
        Self: Sized,
        Self: Iterator,
        <Self as Iterator>::Item: WithTransactions,
    {
        self.into_fallible()
            .bench_txs()
            .map_err(|_e| ())
            .expect("infallible iterator somehow returned an error");
    }
}

impl<I> IteratorExt for I where I: Iterator {}

pub trait FallibleIteratorExt {
    fn bench_txs(self) -> Result<(), Self::Error>
    where
        Self: FallibleIterator,
        Self::Item: WithTransactions;
    fn bench_items(self) -> Result<(), Self::Error>
    where
        Self: FallibleIterator;
}

impl<I> FallibleIteratorExt for I
where
    I: FallibleIterator,
{
    fn bench_txs(mut self) -> Result<(), <Self as FallibleIterator>::Error>
    where
        I::Item: WithTransactions,
    {
        let start = Instant::now();
        let mut last = Instant::now();
        let mut blocks = 0;
        let mut txs = 0;
        let mut blocks_total = 0;
        let mut txs_total = 0;
        while let Some(item) = self.next()? {
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
        Ok(())
    }

    fn bench_items(mut self) -> Result<(), <Self as FallibleIterator>::Error> {
        let start = Instant::now();
        let mut last = Instant::now();
        let mut blocks = 0;
        let mut blocks_total = 0;
        while let Some(_item) = self.next()? {
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
        Ok(())
    }
}
