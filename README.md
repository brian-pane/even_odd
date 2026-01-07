# Warnings

**PLEASE DO NOT USE THIS CRATE FOR ANYTHING IMPORTANT**

This crate checks whether a number is even or odd, in an intentionally
inefficient manner. It started as a joke, but it proved to be an interesting
way to explore things like:

* What loop patterns does `rustc` optimize using SIMD instructions? 
* How much overhead do different threading idioms have for parallelizing
  lightweight computations?
* How does the speed of GPU compute compare to that of CPU compute?

I.e., the code herein takes a very inefficient algorithm and tries to
make it run faster by exploiting the parallelism available in modern
computers.

If you want to check the evenness or oddness of an integer, please just
inspect its low order bit, and do not use this crate.


# Usage

To build the crate and run the performance benchmark tests, run:

```
cargo bench
```

# Sample Performance Results

The following measurements were obtained on a Mac M4 combined CPU+GPU with `rustc` version 1.92.0.

## Scalar Implementation

As a starting point, we have the scalar implementation that loops over all the known even integers from `0` to `2^32 - 1`
and compares each to the target `num`.
```rust
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
```

To keep the compiler from doing any SIMD vectorization tricks just yet, we can test with
`RUSTFLAGS="-Cno-vectorize-loops" cargo bench`. The result is:
```
is_even(u32::MAX)       time:   [481.56 ms 481.92 ms 482.47 ms]
```

(Note: The timings, produced by the `Criterion` benchmarking crate, show `[min, mean, max]`
across many test runs.)

This implementation can determine whether a number is even or odd in about half a second.

## Vector Implementation

But if we do let the compiler vectorize the loop, by compiling without the `RUSTFLAGS="-Cno-vectorize-loops"`,
the result looks like this:
```
is_even(u32::MAX)       time:   [90.312 ms 90.749 ms 91.415 ms]
```

This 5x improvement in speed comes from two optimizations the compiler does during code generation:
1. Vectorization: The AArch64 (aka arm64) target platform has 128-bit wide SIMD registers that each can
   hold 4 `u32` values, so we can compare the input number to 4 known even numbers at once. 
2. Loop unrolling: Because the target platform has a large number of 128-bit vector registers, the
   compiler can maintain multiple registers full of known even numbers and do even more branchless
   comparisons per loop iteration.

## Multi-Threaded Implementation

The compiler has achieved an impressive speedup by auto-vectorizing the code. But the target system
has 10 CPU cores, 9 of which aren't yet participating in the computation.

The `Rayon` rust crate makes it straightforward to set up a pool of worker threads, one per CPU core,
and split the work among them. The Rayon-based version of the code looks like this:
```rust
pub fn is_even_rayon(num: u32) -> bool {
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

```


With vectorization plus parallelization across threads, the benchmark results improve to:
```
is_even_rayon(u32::MAX) time:   [15.075 ms 15.291 ms 15.572 ms]
```

Note that this is only a 6x speedup, despite running on 10x as many cores as the single-threaded
benchmark. The reason is that the CPU has a combination of 4 fast "performance cores" and 6 slower
"efficiency cores," and the system appears to have used one of the performance cores when running
the single-threaded test.

## GPU Implementation

The combination of vectorization and multi-threading has provided a 31x speedup compared to the
baseline, but there is additional parallelism available in the system's GPU. The `wgpu` crate
provides an interface to run portable code on the GPU.

The GPU implementation, written in WGSL, looks like this:
```wgsl
@group(0) @binding(0)
var<storage, read> input: u32;

@group(0) @binding(1)
var<storage, read_write> output: u32;

@compute @workgroup_size(64)
fn is_even(@builtin(global_invocation_id) global_id: vec3<u32>) {
    const BATCH_SIZE = 131072u;
    var known_even = global_id.x * BATCH_SIZE;
    var haystack = vec4u(known_even, known_even + 2, known_even + 4, known_even + 6);
    var needle = vec4u (input, input, input, input);
    const step = vec4u(8, 8, 8, 8);
    var count = BATCH_SIZE / 8;
    for (; count != 0; count -= 1) {
        if any(needle == haystack) {
            output = 1;
        }
        haystack += step;
    }
}
```
The WGPU framework breaks the problem space into _workgroups_ to run in parallel on the GPU.
We have 2^32 numbers to compare against our input, and WGPU limits the number of workgroups
to 65535, so each workgroup handles a range of 2^17 numbers. The compute shader function that
processes a workgroup is similar to the thread worker function from the multi-threaded CPU
implementation, except that the WGSL version is manually vectorized.

The WGPU implementation runs faster than the multi-threaded CPU version on the test system:
```
is_even_wgpu(u32::MAX)  time:   [8.4117 ms 8.5375 ms 8.6627 ms]
```
