//! DynQueue - dynamically extendable Rayon parallel iterator
//!
//! A `DynQueue<T>` can be iterated with `into_par_iter` producing `(DynQueueHandle, T)` elements.
//! With the `DynQueueHandle<T>` a new `T` can be inserted in the `DynQueue<T>`,
//! which is currently iterated over.
//!
//! # Example
//!
//! ```
//! use dynqueue::{DynQueue, DynQueueHandle};
//!
//! use rayon::iter::IntoParallelIterator as _;
//! use rayon::iter::ParallelIterator as _;
//!
//! let mut result = DynQueue::new(vec![1, 2, 3])
//!                      .into_par_iter()
//!                      .map(|(handle, value)| { if value == 2 { handle.enqueue(4) }; value })
//!                      .collect::<Vec<_>>();
//! result.sort();
//!
//! assert_eq!(result, vec![1, 2, 3, 4]);
//! ```
//!
//! # Panics
//!
//! The `DynQueueHandle` shall not outlive the `DynQueue` iterator
//!
//! ```should_panic
//! use dynqueue::{DynQueue, DynQueueHandle};
//!
//! use rayon::iter::IntoParallelIterator as _;
//! use rayon::iter::ParallelIterator as _;
//!
//! static mut STALE_HANDLE : Option<DynQueueHandle<u8, Vec<u8>>> = None;
//!
//! pub fn test_func() -> Vec<u8> {
//!     DynQueue::new(vec![1u8, 2u8, 3u8])
//!         .into_par_iter()
//!         .map(|(handle, value)| unsafe { STALE_HANDLE.replace(handle); value })
//!         .collect::<Vec<_>>()
//! }
//! // test_func() panics
//! let result = test_func();
//! unsafe { STALE_HANDLE.as_ref().unwrap().enqueue(4); }
//! ```

#![deny(clippy::all)]
#![deny(missing_docs)]

use rayon::iter::plumbing::{
    bridge_unindexed, Consumer, Folder, UnindexedConsumer, UnindexedProducer,
};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

#[cfg(test)]
mod tests;

/// Everything implementing `Queue` can be handled by DynQueue
pub trait Queue<T>
where
    Self: Sized,
{
    /// push an element in the queue
    fn push(&mut self, v: T);

    /// pop an element from the queue
    fn pop(&mut self) -> Option<T>;

    /// number of elements in the queue
    fn len(&self) -> usize;

    /// split off `size` elements
    fn split_off(&mut self, size: usize) -> Self;
}

impl<T> Queue<T> for Vec<T> {
    #[inline(always)]
    fn push(&mut self, v: T) {
        Vec::push(self, v)
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<T> {
        Vec::pop(self)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        Vec::len(self)
    }

    #[inline(always)]
    fn split_off(&mut self, size: usize) -> Self {
        Vec::split_off(self, size)
    }
}

impl<T> Queue<T> for VecDeque<T> {
    #[inline(always)]
    fn push(&mut self, v: T) {
        VecDeque::push_back(self, v)
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<T> {
        VecDeque::pop_front(self)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        VecDeque::len(self)
    }

    #[inline(always)]
    fn split_off(&mut self, size: usize) -> Self {
        VecDeque::split_off(self, size)
    }
}

#[cfg(feature = "crossbeam-queue")]
use crossbeam_queue::SegQueue;

#[cfg(feature = "crossbeam-queue")]
impl<T> Queue<T> for SegQueue<T> {
    #[inline(always)]
    fn push(&mut self, v: T) {
        SegQueue::push(self, v);
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<T> {
        SegQueue::pop(self).ok()
    }

    #[inline(always)]
    fn len(&self) -> usize {
        SegQueue::len(self)
    }

    #[inline(always)]
    fn split_off(&mut self, size: usize) -> Self {
        let q = SegQueue::new();
        (0..size)
            .filter_map(|_| self.pop())
            .for_each(|ele| q.push(ele));
        q
    }
}

// PhantomData should prevent `DynQueueInner` to outlive the original `DynQueue`
// but does not always.
struct DynQueueInner<'a, T, U: Queue<T>>(std::sync::RwLock<U>, PhantomData<&'a T>);

/// The `DynQueueHandle` returned by the iterator in addition to `T`
pub struct DynQueueHandle<'a, T, U: Queue<T>>(Arc<DynQueueInner<'a, T, U>>);

impl<'a, T, U: Queue<T>> DynQueueHandle<'a, T, U> {
    /// Enqueue `T` in the `DynQueue<T>`, which is currently iterated.
    #[inline]
    pub fn enqueue(&self, job: T) {
        (self.0).0.write().unwrap().push(job)
    }
}

/// The `DynQueue<T>` which can be parallel iterated over
pub struct DynQueue<'a, T, U: Queue<T>>(Arc<DynQueueInner<'a, T, U>>);

impl<'a, T, U: Queue<T>> DynQueue<'a, T, U>
where
    T: Send + Sync,
    U: Queue<T> + Send + Sync,
{
    /// Create a new `DynQueue<T>` from a `Vec<T>`
    #[inline]
    pub fn new(lifo: U) -> Self {
        Self(Arc::new(DynQueueInner(RwLock::new(lifo), PhantomData)))
    }
}

impl<'a, T, U> UnindexedProducer for DynQueue<'a, T, U>
where
    T: Send + Sync,
    U: Queue<T> + Send + Sync,
{
    type Item = (DynQueueHandle<'a, T, U>, T);

    fn split(self) -> (Self, Option<Self>) {
        let len = {
            let q = (self.0).0.read().unwrap();
            q.len()
        };
        if len >= 2 {
            let new_q = {
                let mut q = (self.0).0.write().unwrap();
                let split_off = q.split_off(len / 2);
                DynQueue::new(split_off)
            };
            (self, Some(new_q))
        } else {
            (self, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        let mut folder = folder;
        loop {
            let ret = {
                let mut q = (self.0).0.write().unwrap();
                q.pop()
            };

            if let Some(v) = ret {
                folder = folder.consume((DynQueueHandle(self.0.clone()), v));

                if folder.full() {
                    break;
                }
            } else {
                // Self shall have the only reference
                assert_eq!(Arc::strong_count(&self.0), 1, "Stale Handle");
                break;
            }
        }
        folder
    }
}

impl<'a, T, U> rayon::iter::ParallelIterator for DynQueue<'a, T, U>
where
    T: Send + Sync,
    U: Queue<T> + Send + Sync,
{
    type Item = (DynQueueHandle<'a, T, U>, T);

    fn drive_unindexed<C>(self, consumer: C) -> <C as Consumer<Self::Item>>::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(self, consumer)
    }
}
