//! DynQueue - a parallel work queue that can grow dynamically while it is drained.
//!
//! A [`DynQueue<T>`] is processed in parallel with [`DynQueue::for_each_dyn`].
//! The callback is handed a [`DynQueueHandle`] with which it can enqueue new `T`s.
//! Those enqueued items are *not* stuck on the thread that produced them: every
//! item lives in one shared worklist, and any idle worker drains it. So a workload
//! that generates more work while running is actually load-balanced across all
//! threads, unlike a static Rayon split.
//!
//! # Example
//!
//! ```
//! use dynqueue::IntoDynQueue as _;
//!
//! let out = std::sync::Mutex::new(Vec::new());
//! vec![1, 2, 3]
//!     .into_dyn_queue()
//!     .for_each_dyn(|handle, value| {
//!         if value == 2 {
//!             handle.enqueue(4)
//!         }
//!         out.lock().unwrap().push(value);
//!     });
//!
//! let mut result = out.into_inner().unwrap();
//! result.sort();
//! assert_eq!(result, vec![1, 2, 3, 4]);
//! ```
//!
//! # Safety
//!
//! `DynQueueHandle` is a *borrowing* handle: it borrows the queue, so it can only
//! live inside the callback it was given to and cannot be smuggled out of the
//! iteration. Trying to retain it is a compile error, not a runtime panic.

#![deny(clippy::all)]
#![deny(missing_docs)]

#[allow(unused)]
macro_rules! doc_comment {
    ($x:expr) => {
        #[doc = $x]
        #[doc(hidden)]
        mod readme_tests {}
    };
}

doc_comment!(include_str!("../README.md"));

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

#[cfg(test)]
mod tests;

/// Convert a collection into a [`DynQueue`].
pub trait IntoDynQueue<T, U: Queue<T>> {
    /// Turn `self` into a `DynQueue<T>`.
    fn into_dyn_queue(self) -> DynQueue<T, U>;
}

/// The back-end storage a [`DynQueue`] is built on.
///
/// Iterators over the items must be implemented as a [`Queue`] so the parallel
/// drain can pull work from a single shared worklist.
pub trait Queue<T>: Send + Sync {
    /// Enqueue an element at the back.
    ///
    /// This must be safe to call from several worker threads at once.
    fn push(&self, v: T);

    /// Remove and return an element, or `None` if the queue is empty.
    fn pop(&self) -> Option<T>;
}

impl<T: Send + Sync> IntoDynQueue<T, RwLock<Vec<T>>> for Vec<T> {
    #[inline(always)]
    fn into_dyn_queue(self) -> DynQueue<T, RwLock<Vec<T>>> {
        DynQueue(DynQueueInner::new(RwLock::new(self)))
    }
}

impl<T: Send + Sync> IntoDynQueue<T, RwLock<Vec<T>>> for RwLock<Vec<T>> {
    #[inline(always)]
    fn into_dyn_queue(self) -> DynQueue<T, RwLock<Vec<T>>> {
        DynQueue(DynQueueInner::new(self))
    }
}

impl<T: Send + Sync> Queue<T> for RwLock<Vec<T>> {
    #[inline(always)]
    fn push(&self, v: T) {
        self.write().unwrap().push(v)
    }

    #[inline(always)]
    fn pop(&self) -> Option<T> {
        self.write().unwrap().pop()
    }
}

impl<T: Send + Sync> IntoDynQueue<T, RwLock<VecDeque<T>>> for VecDeque<T> {
    #[inline(always)]
    fn into_dyn_queue(self) -> DynQueue<T, RwLock<VecDeque<T>>> {
        DynQueue(DynQueueInner::new(RwLock::new(self)))
    }
}

impl<T: Send + Sync> IntoDynQueue<T, RwLock<VecDeque<T>>> for RwLock<VecDeque<T>> {
    #[inline(always)]
    fn into_dyn_queue(self) -> DynQueue<T, RwLock<VecDeque<T>>> {
        DynQueue(DynQueueInner::new(self))
    }
}

impl<T: Send + Sync> Queue<T> for RwLock<VecDeque<T>> {
    #[inline(always)]
    fn push(&self, v: T) {
        self.write().unwrap().push_back(v)
    }

    #[inline(always)]
    fn pop(&self) -> Option<T> {
        self.write().unwrap().pop_front()
    }
}

#[cfg(feature = "crossbeam-queue")]
use crossbeam_queue::SegQueue;

#[cfg(feature = "crossbeam-queue")]
impl<T: Send + Sync> IntoDynQueue<T, SegQueue<T>> for SegQueue<T> {
    #[inline(always)]
    fn into_dyn_queue(self) -> DynQueue<T, SegQueue<T>> {
        DynQueue(DynQueueInner::new(self))
    }
}

#[cfg(feature = "crossbeam-queue")]
impl<T: Send + Sync> Queue<T> for SegQueue<T> {
    #[inline(always)]
    fn push(&self, v: T) {
        SegQueue::push(self, v);
    }

    #[inline(always)]
    fn pop(&self) -> Option<T> {
        SegQueue::pop(self)
    }
}

/// A handle to enqueue more work into a [`DynQueue`] while it is being drained.
///
/// It borrows the queue, so it cannot outlive the `for_each_dyn` callback it was
/// received from. It is small and cheap to copy.
#[derive(Clone, Copy)]
pub struct DynQueueHandle<'a, T, U: Queue<T>>(&'a DynQueueInner<T, U>);

impl<T, U: Queue<T>> DynQueueHandle<'_, T, U> {
    /// Enqueue `job`, to be picked up by any idle worker.
    #[inline]
    pub fn enqueue(&self, job: T) {
        let _gate = self.0.gate.lock().unwrap();
        self.0.queue.push(job)
    }
}

/// The parallel work queue produced by [`IntoDynQueue::into_dyn_queue`].
pub struct DynQueue<T, U: Queue<T>>(DynQueueInner<T, U>);

impl<T, U> DynQueue<T, U>
where
    T: Send + Sync,
    U: Queue<T>,
{
    /// Process every element of the queue in parallel, allowing `f` to enqueue
    /// new elements via the [`DynQueueHandle`] it receives.
    ///
    /// Work produced while the queue is being drained is distributed across all
    /// worker threads: they all pull from one shared worklist, so no producer
    /// thread is ever forced to process all of its own newly generated work.
    ///
    /// # Panics
    ///
    /// Panics if the callback panics. The panic is propagated to the caller.
    pub fn for_each_dyn<F>(self, f: F)
    where
        F: Fn(DynQueueHandle<'_, T, U>, T) + Sync + Send,
    {
        let inner = self.0;
        let workers = rayon::current_num_threads();

        // A drainer per worker thread. All drainers share the one worklist, so
        // an item enqueued by any drainer is picked up by the next idle one.
        rayon::scope(|scope| {
            let inner = &inner;
            let f = &f;
            for _ in 0..workers {
                scope.spawn(move |_| drain(inner, f));
            }
        });
    }
}

/// Counts items currently being processed (popped from the queue, not yet
/// returned), so the drain loops can tell when all outstanding work, including
/// work other threads are about to enqueue, is truly finished.
struct InFlight<'a>(&'a AtomicUsize);

impl<'a> InFlight<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        InFlight(counter)
    }
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        // Decrement even if the callback panics, so no worker hangs waiting for
        // quiescence after a panicked item.
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The result of a worker trying to grab the next item.
enum Pop<T> {
    /// An item was taken.
    Item(T),
    /// The queue is temporarily empty but other items are in flight; retry.
    Retry,
    /// The queue is empty and no items are in flight: all work is done.
    Done,
}

/// One worker thread's drain loop. All workers share `inner.queue` (guarded by
/// `inner.gate`), so newly enqueued items are seen by every worker, not just the
/// one that produced them.
fn drain<T, U, F>(inner: &DynQueueInner<T, U>, f: &F)
where
    T: Send,
    U: Queue<T>,
    F: Fn(DynQueueHandle<'_, T, U>, T) + Sync,
{
    loop {
        let decision = {
            let _gate = inner.gate.lock().unwrap();
            if let Some(v) = inner.queue.pop() {
                Pop::Item(v)
            } else if inner.active.load(Ordering::SeqCst) == 0 {
                Pop::Done
            } else {
                Pop::Retry
            }
        };

        match decision {
            // Kept alive until the callback returns, so `active` reflects an
            // item that is genuinely in flight.
            Pop::Item(v) => {
                let _inflight = InFlight::new(&inner.active);
                f(DynQueueHandle(inner), v);
            }
            Pop::Retry => std::thread::yield_now(),
            Pop::Done => break,
        }
    }
}

struct DynQueueInner<T, U: Queue<T>> {
    queue: U,
    /// Serialises `pop`/`enqueue`/quiescence checks so the "empty && idle"
    /// termination decision is race-free.
    gate: Mutex<()>,
    /// Number of items currently being processed (see [`InFlight`]).
    active: AtomicUsize,
    /// Keeps `T`'s type available for the trait bound `U: Queue<T>`.
    marker: PhantomData<T>,
}

impl<T, U: Queue<T>> DynQueueInner<T, U> {
    fn new(queue: U) -> Self {
        DynQueueInner {
            queue,
            gate: Mutex::new(()),
            active: AtomicUsize::new(0),
            marker: PhantomData,
        }
    }
}
