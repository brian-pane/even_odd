# Warnings

**PLEASE DO NOT USE THIS CRATE FOR ANYTHING IMPORTANT**

This crate checks whether a number is even or odd, in an intentionally
inefficient manner. It started as a joke, but it proved to be an interesting
way to explore things like:

* What loop patterns does `rustc` optimize using SIMD instructions? 
* How much overhead do different threading idioms have for parallelizing
  lightweight computations?

I.e., the code herein takes a very inefficient algorithm and tries to
make it run faster by exploiting the parallelism available in modern
CPUs.

If you want to check the evenness or oddness of an integer, please just
inspect its low order bit, and do not use this crate.


# Usage

To build the crate and run the performance benchmark tests, run:

```
cargo bench
```
