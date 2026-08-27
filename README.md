[![Rust](https://github.com/haraldh/dynqueue/workflows/Rust/badge.svg)](https://github.com/haraldh/dynqueue/actions)
[![Coverage Status](https://codecov.io/gh/haraldh/dynqueue/graph/badge.svg?token=E2KO8O9W9O)](https://codecov.io/gh/haraldh/dynqueue)

# DynQueue - a parallel work queue that can grow dynamically while it drains

DynQueue is a small Rust library for processing a worklist in parallel where **new
work can be added while the queue is still being consumed**.

Rayon can express dynamically generated work with scoped tasks, but its ordinary
parallel iterators are designed around splitting an existing collection. DynQueue
provides a different, queue-oriented interface: every item, both initial and newly
enqueued, is processed by the same callback. The queue is drained to completion
without requiring callers to create a task closure for every item or implement
worker lifecycle and termination detection.

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
- Classic *worklist algorithms* (compiler dataflow analyses, mark-and-sweep,
  reachability): the canonical shape is exactly one callback over a queue of
  homogeneous items, where processing an item adds more items — usually
  serially, and DynQueue is the drop-in way to parallelize it.

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

Runnable examples live in [`examples/`](examples/). `dua` is a tiny parallel
`du`: one callback walks the tree, enqueuing the entries of every directory it
reads, and reports filesystem errors as work items instead of panicking.

```sh
cargo run --release --example dua -- --dirs /usr
```

## Compared with a plain Rayon parallel iterator

A dynamic workload processed with a naive `par_iter` split runs at roughly the
speed of one thread. DynQueue distributes newly generated items across all
workers:

| approach                       | wall time (4 threads, heavy dynamic workload) |
| ------------------------------ | --------------------------------------------- |
| strictly sequential            | 1.26 s                                        |
| static Rayon split             | 1.26 s                                        |
| **DynQueue `for_each_dyn`**   | ~0.32 s (≈ 4×)                                |

## How this relates to Rayon

Rayon's `ParallelIterator` contract is built around `split` + `fold`: work is
partitioned up front and each partition is consumed by one thread. Dynamically
enqueued items cannot be re-partitioned once folding starts, so they stay on the
producing thread. DynQueue instead drains one shared, mutex-guarded worklist from
all workers. This delivers load balancing for dynamically generated items, at the
cost of not composing with `map`/`collect` chains.

This does not mean Rayon itself is limited to static work. [`rayon::scope`]
lets a task spawn more tasks, and Rayon's work-stealing scheduler distributes them
among workers. Prefer that API when each generated job is naturally a distinct
closure or needs its own captured state. Prefer DynQueue when the workload is a
stream of homogeneous values handled by one callback:

```text
Rayon scope: task -> spawn another closure
DynQueue:     item -> enqueue another item -> same callback
```

Lower-level concurrent and work-stealing queue crates can also implement this
pattern, but require the caller to construct the worker loop and decide when all
recursively generated work is complete. DynQueue packages those pieces into
`for_each_dyn` while allowing custom storage through the `Queue` trait.

[`rayon::scope`]: https://docs.rs/rayon/latest/rayon/fn.scope.html

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
