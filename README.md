[![Rust](https://github.com/haraldh/dynqueue/workflows/Rust/badge.svg)](https://github.com/haraldh/dynqueue/actions)
[![Coverage Status](https://coveralls.io/repos/github/haraldh/dynqueue/badge.svg?branch=master)](https://coveralls.io/github/haraldh/dynqueue?branch=master)

# DynQueue - dynamically extendable Rayon parallel iterator

A `DynQueue<T>` can be iterated with `into_par_iter` producing `(DynQueueHandle, T)` elements.
With the `DynQueueHandle<T>` a new `T` can be inserted in the `DynQueue<T>`,
which is currently iterated over.

```rust
use dynqueue::DynQueue;

use rayon::iter::IntoParallelIterator as _;
use rayon::iter::ParallelIterator as _;

fn main() {
    let mut result = DynQueue::new(vec![1, 2, 3])
                         .into_par_iter()
                         .map(|(handle, value)| { if value == 2 { handle.enqueue(4) }; value })
                         .collect::<Vec<_>>();
    result.sort();

    assert_eq!(result, vec![1, 2, 3, 4]);
}
```
