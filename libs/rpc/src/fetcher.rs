use crate::Rpc;
use anyhow::Result;
use block_iter_core::{BlockHash, BlockHeight, WithHeightAndId, WithPrevBlockHash};
use log::{debug, info, trace};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

/// Retry a failing rpc
fn retry<T>(mut f: impl FnMut() -> Result<T>) -> T {
    let delay_ms = 100;
    let mut count = 0;
    loop {
        match f() {
            Err(e) => {
                std::thread::sleep(Duration::from_millis(delay_ms));
                if count % 1000 == 0 {
                    eprintln!("{}; retrying ...", e);
                }
                count += 1;
            }
            Ok(t) => {
                return t;
            }
        }
    }
}

/// A block fetcher from a `Rpc`
///
/// Implemented as an iterator that yields block events in order,
/// and blocks for new ones when needed.
///
/// It uses thread-pool to fetch blocks and returns them in order:
///
/// ```norust
/// 1, 2, 3, ..., n-1, n ...
/// ```
///
/// In case of an reorg, it will break the sequence and return
/// the right blocks of a new chain, again in sequence:
///
/// ```norust
/// 1, 2, 3, 4, ..., 2, 3, 4 ...
/// ```
/// # Architecture notes
///
/// Note that prefetcher does not have any access to the DB or
/// persistent storage, so it does not know what have been previous
/// indexed. It makes using it a bit more diffucult,
/// but the benefit is that it's much more composable and isolated.
///
/// In a sense, the `Fetcher` is a simplest and smallest-possible
/// `Indexer`, that just does not actually index anything. It only
/// fetches blocks and detects reorgs.
pub struct Fetcher<R>
where
    R: Rpc,
{
    rx: Option<crossbeam_channel::Receiver<WithHeightAndId<R::Data>>>,
    /// Worker threads
    thread_joins: Vec<std::thread::JoinHandle<()>>,
    /// List of blocks that arrived out-of-order: before the block
    /// we were actually waiting for.
    out_of_order_items: HashMap<BlockHeight, WithHeightAndId<R::Data>>,

    cur_height: BlockHeight,
    prev_hashes: BTreeMap<BlockHeight, BlockHash>,
    workers_finish: Arc<AtomicBool>,
    thread_num: usize,
    rpc: Arc<R>,
    end_of_fast_sync: BlockHeight,
}

impl<R> Fetcher<R>
where
    R: Rpc + 'static,
    R::Data: WithPrevBlockHash,
{
    pub fn new(rpc: Arc<R>, last_block: Option<WithHeightAndId<R::Data>>) -> Result<Self> {
        let thread_num = 8;
        let workers_finish = Arc::new(AtomicBool::new(false));

        let end_of_fast_sync = retry(|| rpc.get_block_count());
        let mut prev_hashes = BTreeMap::default();
        let start = if let Some(h_and_hash) = last_block {
            let h = h_and_hash.height;
            prev_hashes.insert(h, h_and_hash.id);
            info!("Starting block fetcher starting at {}H", h + 1);
            h + 1
        } else {
            info!("Starting block fetcher starting at genesis block");
            0
        };

        let mut s = Self {
            rx: None,
            rpc,
            thread_joins: Default::default(),
            thread_num,
            cur_height: start,
            out_of_order_items: Default::default(),
            workers_finish,
            prev_hashes,
            end_of_fast_sync,
        };

        s.start_workers();
        Ok(s)
    }

    fn start_workers(&mut self) {
        self.workers_finish.store(false, Ordering::SeqCst);

        let (tx, rx) = crossbeam_channel::bounded(self.thread_num * 64);
        self.rx = Some(rx);
        let next_height = Arc::new(AtomicUsize::new(self.cur_height as usize));
        assert!(self.thread_joins.is_empty());
        for _ in 0..self.thread_num {
            self.thread_joins.push({
                std::thread::spawn({
                    let next_height = next_height.clone();
                    let rpc = self.rpc.clone();
                    let tx = tx.clone();
                    let workers_finish = self.workers_finish.clone();
                    let in_progress = Arc::new(Mutex::new(Default::default()));
                    move || {
                        // TODO: constructor
                        let mut worker = Worker {
                            next_height,
                            workers_finish,
                            rpc,
                            tx,
                            in_progress,
                        };

                        worker.run()
                    }
                })
            });
        }
    }

    /// Detect reorgs
    ///
    /// Track previous hashes and detect if a given block points
    /// to a different `prev_blockhash` than we recorded. That
    /// means that the previous hash we've recorded was abandoned.
    fn track_reorgs(&mut self, block: &WithHeightAndId<R::Data>) -> bool {
        debug_assert_eq!(block.height, self.cur_height);
        if self.cur_height > 0 {
            if let Some(stored_prev_id) = self.prev_hashes.get(&(self.cur_height - 1)) {
                trace!(
                    "Reorg check: last_id {} =? current {} at {}H",
                    stored_prev_id,
                    block.data.prev_block_hash(),
                    self.cur_height - 1
                );
                if stored_prev_id != block.data.prev_block_hash() {
                    return true;
                }
            } else if self.cur_height
                < *self
                    .prev_hashes
                    .iter()
                    .next()
                    .expect("At least one element")
                    .0
            {
                panic!(
                    "Fetcher detected a reorg beyond acceptable depth. No hash for {}H",
                    self.cur_height
                );
            } else {
                let max_prev_hash = self
                    .prev_hashes
                    .iter()
                    .next_back()
                    .expect("At least one element");
                if self.cur_height != *max_prev_hash.0 + 1 {
                    for (h, hash) in self.prev_hashes.iter() {
                        debug!("prev_hash {}H -> {}", h, hash);
                    }
                    panic!(
                        "No prev_hash for a new block {}H {}; max_prev_hash: {}H {}",
                        self.cur_height, block.id, max_prev_hash.0, max_prev_hash.1
                    );
                }
            }
        }
        self.prev_hashes.insert(block.height, block.id.clone());
        // this is how big reorgs we're going to detect
        let window_size = 1000;
        if self.cur_height >= window_size {
            self.prev_hashes.remove(&(self.cur_height - window_size));
        }
        assert!(self.prev_hashes.len() <= window_size as usize);

        false
    }

    /// Handle condition detected by `detected_reorg`
    ///
    /// Basically, stop all workers (discarding their work), adjust height and
    /// start workers again.
    ///
    /// This doesn't have to be blazing fast, so it isn't.
    fn reset_on_reorg(&mut self) {
        debug!(
            "Resetting on reorg from {}H to {}H",
            self.cur_height,
            self.cur_height - 1
        );
        self.stop_workers();
        assert!(self.cur_height > 0);
        self.cur_height -= 1;
        self.start_workers();
    }
}

impl<R> Fetcher<R>
where
    R: Rpc,
{
    fn stop_workers(&mut self) {
        self.workers_finish.store(true, Ordering::SeqCst);

        while let Ok(_) = self
            .rx
            .as_ref()
            .expect("start_workers called before stop_workers")
            .recv()
        {}

        self.rx = None;
        self.thread_joins.drain(..).map(|j| j.join()).for_each(drop);
        self.out_of_order_items.clear();
    }
}

impl<R> Iterator for Fetcher<R>
where
    R: Rpc + 'static,
    R::Data: WithPrevBlockHash,
{
    type Item = WithHeightAndId<R::Data>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.end_of_fast_sync == self.cur_height {
            debug!(
                "Fetcher: end of fast sync at {}H; switching to one worker",
                self.cur_height
            );
            self.stop_workers();
            self.thread_num = 1;
            self.start_workers();
        }

        'retry_on_reorg: loop {
            if let Some(item) = self.out_of_order_items.remove(&self.cur_height) {
                if self.track_reorgs(&item) {
                    self.reset_on_reorg();
                    continue 'retry_on_reorg;
                }
                self.cur_height += 1;
                return Some(item);
            }

            loop {
                trace!(
                    "Waiting for the block from the workers at: {}H",
                    self.cur_height
                );
                let item = self
                    .rx
                    .as_ref()
                    .expect("rx available")
                    .recv()
                    .expect("Workers shouldn't disconnect");
                trace!("Got the block from the workers from: {}H", item.height);
                if item.height == self.cur_height {
                    if self.track_reorgs(&item) {
                        self.reset_on_reorg();
                        continue 'retry_on_reorg;
                    }
                    self.cur_height += 1;
                    return Some(item);
                } else {
                    assert!(item.height > self.cur_height);
                    self.out_of_order_items.insert(item.height, item);
                }
            }
        }
    }
}

impl<R> Drop for Fetcher<R>
where
    R: Rpc,
{
    fn drop(&mut self) {
        self.stop_workers();
    }
}

/// One worker thread, polling for data from the node
struct Worker<R>
where
    R: Rpc,
    R::Data: WithPrevBlockHash,
{
    rpc: Arc<R>,
    next_height: Arc<AtomicUsize>,
    workers_finish: Arc<AtomicBool>,
    tx: crossbeam_channel::Sender<WithHeightAndId<R::Data>>,
    in_progress: Arc<Mutex<BTreeSet<BlockHeight>>>,
}

impl<R> Worker<R>
where
    R: Rpc,
    R::Data: WithPrevBlockHash,
{
    fn run(&mut self) {
        loop {
            let height = self.get_height_to_fetch();

            let mut retry_count = 0;
            'retry: loop {
                if self.workers_finish.load(Ordering::SeqCst) {
                    return;
                }

                match self.get_block_by_height(height) {
                    Err(e) => {
                        trace!("Error from the node: {}", e);
                        let ahead_minimum = height
                            - self
                                .get_min_height_in_progress()
                                .expect("at least current height");
                        std::thread::sleep(Duration::from_millis(
                            (1 + R::RECOMMENDED_ERROR_RETRY_DELAY_MS) * u64::from(ahead_minimum),
                        ));
                        retry_count += 1;
                        if retry_count % 10 == 0 {
                            debug!("Worker retrying rpc error {} at {}H", e, height);
                        }
                    }
                    Ok(None) => {
                        let sleep_ms = R::RECOMMENDED_HEAD_RETRY_DELAY_MS;
                        std::thread::sleep(Duration::from_millis(sleep_ms));
                    }
                    Ok(Some(item)) => {
                        self.tx.send(item).expect("Send must not fail");
                        self.mark_height_fetched(height);
                        break 'retry;
                    }
                }
            }
        }
    }

    fn get_height_to_fetch(&self) -> BlockHeight {
        let height = self.next_height.fetch_add(1, Ordering::SeqCst) as BlockHeight;
        self.in_progress
            .lock()
            .expect("unlock works")
            .insert(height);
        height
    }

    fn get_min_height_in_progress(&self) -> Option<BlockHeight> {
        let in_progress = self.in_progress.lock().expect("unlock works");
        in_progress.iter().next().cloned()
    }

    fn mark_height_fetched(&self, height: BlockHeight) {
        assert!(self
            .in_progress
            .lock()
            .expect("unlock works")
            .remove(&height));
    }

    fn get_block_by_height(
        &mut self,
        height: BlockHeight,
    ) -> Result<Option<WithHeightAndId<R::Data>>> {
        if let Some(id) = self.rpc.get_block_id_by_height(height)? {
            Ok(self.rpc.get_block_by_id(&id)?.map(|block| WithHeightAndId {
                height,
                id,
                data: block,
            }))
        } else {
            Ok(None)
        }
    }
}
