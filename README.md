[![Rust](https://github.com/haraldh/dynqueue/workflows/Rust/badge.svg)](https://github.com/haraldh/dynqueue/actions)
[![Coverage Status](https://coveralls.io/repos/github/haraldh/dynqueue/badge.svg?branch=master)](https://coveralls.io/github/haraldh/dynqueue?branch=master)

# DynQueue - dynamically extendable Rayon parallel iterator

DynQueue is a Rust library that provides a specialized parallel iterator built on top of the Rayon parallel computing framework. Its key feature is the ability to dynamically add new items to the collection while it's being processed in parallel.

This project fills a specific niche in the Rust parallel computing ecosystem by providing a way to work with dynamically growing workloads in a parallel context, something that's not directly supported by Rayon itself.

## Core Functionality:
- Allows adding new elements to a collection while it's being iterated over in parallel
- Works with multiple collection types: `Vec`, `VecDeque`, and `crossbeam_queue::SegQueue` (via feature flag)
- Uses Rayon's parallel iteration capabilities for efficient parallel processing
- Provides a clean API for dynamically expanding workloads

## Technical Implementation:
- Implements Rayon's parallel iteration traits
- Uses a handle-based approach where each iterator function receives a handle to insert new items
- Thread-safe implementation using RwLock and Arc for standard collections
- Optional integration with crossbeam's lock-free SegQueue

## Use Cases:
The library is particularly useful for:
- Tree/graph traversal algorithms where new nodes are discovered during processing
- Work-stealing parallel algorithms where work can be dynamically generated
- Any parallel processing task where the total workload is not known upfront

## Example Usage:
```rust
use rayon::iter::IntoParallelIterator as _;
use rayon::iter::ParallelIterator as _;
use dynqueue::IntoDynQueue as _;

fn main() {
    let mut result = vec![1, 2, 3]
        .into_dyn_queue()
        .into_par_iter()
        .map(|(handle, value)| { 
            if value == 2 { 
                handle.enqueue(4) 
            }; 
            value 
        })
        .collect::<Vec<_>>();
    result.sort();

    assert_eq!(result, vec![1, 2, 3, 4]);
}
```

## TL;DR

A `DynQueue<T>` can be iterated with `into_par_iter` producing `(DynQueueHandle, T)` elements.
With the `DynQueueHandle<T>` a new `T` can be inserted in the `DynQueue<T>`,
which is currently iterated over.

A `Vec<T>`, `VecDeque<T>` and `crossbeam_queue::SegQueue<T>` (with `feature = "crossbeam-queue"`)
can be turned into a `DynQueue<T>` with `.into_dyn_queue()`.


## Features

* `crossbeam-queue` : to use `crossbeam::queue::SegQueue` as the inner collection.

## Changelog

### 0.2.0
- introduce `IntoDynQueue`
- handle lockless collections

### 0.1.0
- initial version
