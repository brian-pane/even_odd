use rayon::prelude::*;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

pub fn is_even(num: u32) -> bool {
    let mut even = false;
    let mut known_even = Some(0u32);
    while let Some(known) = known_even {
        if num == known {
            even = true;
        }
        known_even = known.checked_add(2);
    }
    even
}

const LOG2_BLOCK_SIZE: usize = 12;

const BLOCK_SIZE: usize = 1 << LOG2_BLOCK_SIZE;

const TOTAL_BLOCKS: usize = 1 << (32 - LOG2_BLOCK_SIZE);

fn partition_chunks(num_chunks: usize) -> Vec<(usize, usize)> {
    let num_chunks = core::cmp::min(num_chunks, TOTAL_BLOCKS);
    let mut chunks = Vec::with_capacity(num_chunks);
    let mut start_block: usize = 0;
    let blocks_per_chunk = TOTAL_BLOCKS / num_chunks;
    for i in 0..num_chunks {
        let num_blocks = if i + 1 == num_chunks {
            TOTAL_BLOCKS - start_block
        } else {
            blocks_per_chunk
        };
        chunks.push((start_block, num_blocks));
        start_block += num_blocks;
    }
    chunks
}

pub fn is_even_rayon(num: u32) -> bool {
    // Split the range [0..u32::MAX] into N chunks, where N = num CPU cores * CHUNK_MULTIPLIER.
    // A CHUNK_MULTIPLIER of 1 is optimal if all CPU cores on the system are equally fast, but
    // if some cores are faster than others (e.g. on a processor with a mix of performance cores
    // and efficiency cores) we often can get better results with a larger number of smaller chunks.
    const CHUNK_MULTIPLIER: usize = 10;
    let chunks =
        partition_chunks(std::thread::available_parallelism().unwrap().get() * CHUNK_MULTIPLIER);
    let even = Arc::new(Mutex::new(false));
    let worker_even = Arc::clone(&even);
    chunks
        .par_iter()
        .for_each(move |&(start_block, num_blocks)| {
            let mut local_even = false;
            let mut known_even = (start_block * BLOCK_SIZE) as u32;
            for _ in 0..BLOCK_SIZE * num_blocks / 2 {
                if known_even == num {
                    local_even = true;
                }
                known_even = known_even.wrapping_add(2);
            }
            if local_even {
                *worker_even.lock().unwrap() = true;
            }
        });

    *even.lock().unwrap()
}

pub struct EvenOdd {
    workers: Vec<JoinHandle<()>>,
    to_workers: Vec<mpsc::Sender<Option<Request>>>,
}

struct Request {
    num: u32,
    tx: mpsc::Sender<bool>,
}

impl EvenOdd {
    pub fn new() -> Self {
        let chunks = partition_chunks(std::thread::available_parallelism().unwrap().get());
        let mut workers = Vec::with_capacity(chunks.len());
        let mut to_workers = Vec::with_capacity(chunks.len());
        for (start_block, num_blocks) in chunks {
            let (tx, rx) = mpsc::channel::<Option<Request>>();
            to_workers.push(tx);
            workers.push(thread::spawn(move || {
                loop {
                    let received = rx.recv().unwrap();
                    match received {
                        None => break,
                        Some(request) => {
                            let mut local_even = false;
                            let mut known_even = (start_block * BLOCK_SIZE) as u32;
                            for _ in 0..BLOCK_SIZE * num_blocks / 2 {
                                if known_even == request.num {
                                    local_even = true;
                                }
                                known_even = known_even.wrapping_add(2);
                            }
                            request.tx.send(local_even).unwrap();
                        }
                    }
                }
            }));
        }
        Self {
            workers,
            to_workers,
        }
    }

    pub fn is_even(&self, num: u32) -> bool {
        let mut from_workers = Vec::with_capacity(self.to_workers.len());
        for to_worker in &self.to_workers {
            let (tx, rx) = mpsc::channel::<bool>();
            from_workers.push(rx);
            to_worker.send(Some(Request { num, tx })).unwrap();
        }
        let mut even = false;
        for rx in from_workers {
            let received = rx.recv().unwrap();
            even |= received;
        }
        even
    }
}

impl Default for EvenOdd {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EvenOdd {
    fn drop(&mut self) {
        self.to_workers.iter().for_each(|tx| {
            tx.send(None).unwrap();
        });
        self.workers.drain(..).for_each(|w| w.join().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hint::black_box;

    #[test]
    fn single_threaded() {
        for (num, expected) in [
            (0, true),
            (1, false),
            (2, true),
            (9_999_999, false),
            (u32::MAX, false),
        ] {
            assert_eq!(is_even(black_box(num)), expected);
        }
    }

    #[test]
    fn rayon() {
        for (num, expected) in [
            (0, true),
            (1, false),
            (2, true),
            (9_999_999, false),
            (u32::MAX, false),
        ] {
            assert_eq!(is_even_rayon(black_box(num)), expected);
        }
    }

    #[test]
    fn even_odd() {
        let even_odd = EvenOdd::new();
        for (num, expected) in [
            (0, true),
            (1, false),
            (2, true),
            (9_999_999, false),
            (u32::MAX, false),
        ] {
            assert_eq!(even_odd.is_even(black_box(num)), expected);
        }
    }
}
