[![Rust](https://github.com/haraldh/dynqueue/workflows/Rust/badge.svg)](https://github.com/haraldh/dynqueue/actions)
[![Coverage Status](https://codecov.io/gh/haraldh/dynqueue/graph/badge.svg?token=E2KO8O9W9O)](https://codecov.io/gh/haraldh/dynqueue)

# DynQueue - a parallel work queue that can grow dynamically while it drains

DynQueue is a small Rust library for processing a worklist in parallel where **new
work can be added while the queue is still being consumed**.

This fills a niche Rayon does not cover directly: Rayon's parallel iterators split
a *known, static* collection across threads and cannot rebalance work that is
*generated* as iteration runs. DynQueue instead puts every item (initial *and*
newly enqueued) into one shared worklist that every worker thread drains, so a
workload that spawns more work while running stays load-balanced.

## Core functionality

- Process a queue in parallel with `for_each_dyn`, whose callback can enqueue more
  work via the `DynQueueHandle` it receives.
- Newly enqueued items are picked up by **any** idle worker — they are never stuck
  on the thread that produced them.
- Works over `Vec`, `VecDeque` and `crossbeam_queue::SegQueue` (via the
  `crossbeam-queue` feature). Add your own backing by implementing the `Queue`
  trait.

## Use cases

- Tree / graph traversal where new nodes are discovered while visiting.
- Backtracking and search where each step can produce more candidates.
- Any parallel task whose total workload is not known ahead of time.

## Example

```rust
use dynqueue::IntoDynQueue as _;

let out = std::sync::Mutex::new(Vec::new());
vec![1, 2, 3]
    .into_dyn_queue()
    .for_each_dyn(|handle, value| {
        if value == 2 {
            handle.enqueue(4)
        }
        out.lock().unwrap().push(value);
    });

let mut result = out.into_inner().unwrap();
result.sort();
assert_eq!(result, vec![1, 2, 3, 4]);
```

> Note: this is a **breaking change** (v0.3 → v0.4). The old `into_par_iter()`
> API provided only a static Rayon split and did not distribute dynamically
> generated work; it was replaced by the dynamic `for_each_dyn` (see the
> "Why not a parallel iterator?" section).

## Advantages over a plain Rayon parallel iterator

A dynamic workload processed with a naive `par_iter` split runs at roughly the
speed of one thread. DynQueue distributes newly generated items across all
workers:

| approach                       | wall time (4 threads, heavy dynamic workload) |
| ------------------------------ | --------------------------------------------- |
| strictly sequential            | 1.26 s                                        |
| static Rayon split             | 1.26 s                                        |
| **DynQueue `for_each_dyn`** | ~0.32 s (≈ 4×)                                |

## Why not a Rayon parallel iterator?

Rayon's `ParallelIterator` contract is built around `split` + `fold`: work is
partitioned up front and each partition is consumed by one thread. Dynamically
enqueued items cannot be re-partitioned once folding starts, so they stay on the
producing thread. That made the old `into_par_iter` behind `dynqueue` effectively
sequential for growing workloads. DynQueue sidesteps this by draining one shared,
mutex-guarded worklist from all workers — that is what delivers real load
balancing, at the cost of not composing with `map`/`collect` chains.

## Constraints

- `DynQueueHandle` is a borrowing handle: it is only valid inside the callback it
  is given to. You cannot stash it for later — that is a compile error, not a
  runtime panic.
- The callback must be `Send + Sync` (shared across worker threads). Use a
  `Mutex`/channel to write results out of the callback.
- Panics inside the callback propagate to the caller.

## Features

- `crossbeam-queue`: use `crossbeam::queue::SegQueue` as the back-end.

## TL;DR

A `DynQueue<T>` is drained in parallel with `for_each_dyn(|handle, item| ...)`.
Inside the callback, `handle.enqueue(new)` adds more work that any idle worker
picks up.

## Changelog

### 0.4.0

- **Breaking:** replace the static `into_par_iter` API with `for_each_dyn`.
  Dynamically generated work is now distributed across all worker threads instead
  of being stuck on the producing thread.
- `DynQueueHandle` is now borrowing and cannot outlive the callback.
- Drop the obsolete `Queue::{len, split_off}` methods.

### 0.2.0

- introduce `IntoDynQueue`
- handle lockless collections

### 0.1.0

- initial version
