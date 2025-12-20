use std::sync::{Arc, Mutex};
use rayon::prelude::*;

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

pub fn is_even_parallel(num: u32) -> bool {
    // A block is a fixed, power-of-two size. A chunk consists of one or more blocks.
    const LOG2_BLOCK_SIZE: usize = 12;
    const BLOCK_SIZE: usize = 1 << LOG2_BLOCK_SIZE;
    const TOTAL_BLOCKS: usize = 1 << (32 - LOG2_BLOCK_SIZE);

    let num_chunks = std::thread::available_parallelism().unwrap().get();
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

    let even = Arc::new(Mutex::new(false));
    let worker_even = Arc::clone(&even);
    chunks.par_iter().for_each(move |&(start_block, num_blocks)| {
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

    even.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use core::hint::black_box;
    use super::*;

    #[test]
    fn single_threaded() {
        for (num, expected) in [(0, true), (1, false), (2, true), (9_999_999, false), (u32::MAX, false)] {
            assert_eq!(is_even(black_box(num)), expected);
        }
    }

    #[test]
    fn multi_threaded() {
        for (num, expected) in [(0, true), (1, false), (2, true), (9_999_999, false), (u32::MAX, false)] {
            assert_eq!(is_even_parallel(black_box(num)), expected);
        }
    }
}
