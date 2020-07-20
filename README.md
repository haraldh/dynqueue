[![Rust](https://github.com/haraldh/dynqueue/workflows/Rust/badge.svg)](https://github.com/haraldh/dynqueue/actions)
[![Coverage Status](https://coveralls.io/repos/github/haraldh/dynqueue/badge.svg?branch=master)](https://coveralls.io/github/haraldh/dynqueue?branch=master)

# DynQueue - dynamically extendable Rayon parallel iterator

A `DynQueue<T>` can be iterated with `into_par_iter` producing `(DynQueueHandle, T)` elements.
With the `DynQueueHandle<T>` a new `T` can be inserted in the `DynQueue<T>`,
which is currently iterated over.

A `Vec<T>`, `VecDeque<T>` and `crossbeam_queue::SegQueue<T>` (with `feature = "crossbeam-queue"`)
can be turned into a `DynQueue<T>` with `.into_dyn_queue()`.

```rust
use rayon::iter::IntoParallelIterator as _;
use rayon::iter::ParallelIterator as _;

use dynqueue::IntoDynQueue as _;

fn main() {
    let mut result = vec![1, 2, 3]
        .into_dyn_queue()
        .into_par_iter()
        .map(|(handle, value)| { if value == 2 { handle.enqueue(4) }; value })
        .collect::<Vec<_>>();
    result.sort();

    assert_eq!(result, vec![1, 2, 3, 4]);
}
```

## Features

* `crossbeam-queue` : to use `crossbeam::queue::SegQueue` as the inner collection.

## Changelog

### 0.2.0
- introduce `IntoDynQueue`
- handle lockless collections

### 0.1.0
- initial version
