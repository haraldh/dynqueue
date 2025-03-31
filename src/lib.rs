//! DynQueue - dynamically extendable Rayon parallel iterator
//!
//! A `DynQueue<T>` can be iterated with `into_par_iter` producing `(DynQueueHandle, T)` elements.
//! With the `DynQueueHandle<T>` a new `T` can be inserted in the `DynQueue<T>`,
//! which is currently iterated over.
//!
//! # Example
//!
//! ```
//! use rayon::iter::IntoParallelIterator as _;
//! use rayon::iter::ParallelIterator as _;
//!
//! use dynqueue::IntoDynQueue as _;
//!
//! let mut result = vec![1, 2, 3]
//!     .into_dyn_queue()
//!     .into_par_iter()
//!     .map(|(handle, value)| {
//!         if value == 2 {
//!             handle.enqueue(4)
//!         };
//!         value
//!     })
//!     .collect::<Vec<_>>();
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
//! use dynqueue::{DynQueue, DynQueueHandle, IntoDynQueue};
//!
//! use rayon::iter::IntoParallelIterator as _;
//! use rayon::iter::ParallelIterator as _;
//! use std::sync::RwLock;
//!
//! static mut STALE_HANDLE: Option<DynQueueHandle<u8, RwLock<Vec<u8>>>> = None;
//!
//! pub fn test_func() -> Vec<u8> {
//!     vec![1u8, 2u8, 3u8]
//!         .into_dyn_queue()
//!         .into_par_iter()
//!         .map(|(handle, value)| unsafe {
//!             STALE_HANDLE.replace(handle);
//!             value
//!         })
//!         .collect::<Vec<_>>()
//! }
//! // test_func() panics
//! let result = test_func();
//! unsafe {
//!     STALE_HANDLE.as_ref().unwrap().enqueue(4);
//! }
//! ```

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

use rayon::iter::plumbing::{
    bridge_unindexed, Consumer, Folder, UnindexedConsumer, UnindexedProducer,
};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

#[cfg(test)]
mod tests;

/// Trait to produce a new DynQueue
pub trait IntoDynQueue<T, U: Queue<T>> {
    /// new
    fn into_dyn_queue<'a>(self) -> DynQueue<'a, T, U>;
}

/// Everything implementing `Queue` can be handled by DynQueue
#[allow(clippy::len_without_is_empty)]
pub trait Queue<T>
where
    Self: Sized,
{
    /// push an element in the queue
    fn push(&self, v: T);

    /// pop an element from the queue
    fn pop(&self) -> Option<T>;

    /// number of elements in the queue
    fn len(&self) -> usize;

    /// split off `size` elements
    fn split_off(&self, size: usize) -> Self;
}

impl<T> IntoDynQueue<T, RwLock<Vec<T>>> for Vec<T> {
    #[inline(always)]
    fn into_dyn_queue<'a>(self) -> DynQueue<'a, T, RwLock<Vec<T>>> {
        DynQueue(Arc::new(DynQueueInner(RwLock::new(self), PhantomData)))
    }
}

impl<T> IntoDynQueue<T, RwLock<Vec<T>>> for RwLock<Vec<T>> {
    #[inline(always)]
    fn into_dyn_queue<'a>(self) -> DynQueue<'a, T, RwLock<Vec<T>>> {
        DynQueue(Arc::new(DynQueueInner(self, PhantomData)))
    }
}

impl<T> Queue<T> for RwLock<Vec<T>> {
    #[inline(always)]
    fn push(&self, v: T) {
        self.write().unwrap().push(v)
    }

    #[inline(always)]
    fn pop(&self) -> Option<T> {
        self.write().unwrap().pop()
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.read().unwrap().len()
    }

    #[inline(always)]
    fn split_off(&self, size: usize) -> Self {
        RwLock::new(self.write().unwrap().split_off(size))
    }
}

impl<T> IntoDynQueue<T, RwLock<VecDeque<T>>> for VecDeque<T> {
    #[inline(always)]
    fn into_dyn_queue<'a>(self) -> DynQueue<'a, T, RwLock<VecDeque<T>>> {
        DynQueue(Arc::new(DynQueueInner(RwLock::new(self), PhantomData)))
    }
}

impl<T> IntoDynQueue<T, RwLock<VecDeque<T>>> for RwLock<VecDeque<T>> {
    #[inline(always)]
    fn into_dyn_queue<'a>(self) -> DynQueue<'a, T, RwLock<VecDeque<T>>> {
        DynQueue(Arc::new(DynQueueInner(self, PhantomData)))
    }
}

impl<T> Queue<T> for RwLock<VecDeque<T>> {
    #[inline(always)]
    fn push(&self, v: T) {
        self.write().unwrap().push_back(v)
    }

    #[inline(always)]
    fn pop(&self) -> Option<T> {
        self.write().unwrap().pop_front()
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.read().unwrap().len()
    }

    #[inline(always)]
    fn split_off(&self, size: usize) -> Self {
        RwLock::new(self.write().unwrap().split_off(size))
    }
}

#[cfg(feature = "crossbeam-queue")]
use crossbeam_queue::SegQueue;

#[cfg(feature = "crossbeam-queue")]
impl<T> IntoDynQueue<T, SegQueue<T>> for SegQueue<T> {
    #[inline(always)]
    fn into_dyn_queue<'a>(self) -> DynQueue<'a, T, Self> {
        DynQueue(Arc::new(DynQueueInner(self, PhantomData)))
    }
}

#[cfg(feature = "crossbeam-queue")]
impl<T> Queue<T> for SegQueue<T> {
    #[inline(always)]
    fn push(&self, v: T) {
        SegQueue::push(self, v);
    }

    #[inline(always)]
    fn pop(&self) -> Option<T> {
        SegQueue::pop(self)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        SegQueue::len(self)
    }

    #[inline(always)]
    fn split_off(&self, size: usize) -> Self {
        let q = SegQueue::new();
        (0..size)
            .filter_map(|_| Queue::pop(self))
            .for_each(|ele| q.push(ele));
        q
    }
}

// PhantomData should prevent `DynQueueInner` to outlive the original `DynQueue`
// but does not always.
struct DynQueueInner<'a, T, U: Queue<T>>(U, PhantomData<&'a T>);

/// The `DynQueueHandle` returned by the iterator in addition to `T`
pub struct DynQueueHandle<'a, T, U: Queue<T>>(Arc<DynQueueInner<'a, T, U>>);

impl<T, U: Queue<T>> DynQueueHandle<'_, T, U> {
    /// Enqueue `T` in the `DynQueue<T>`, which is currently iterated.
    #[inline]
    pub fn enqueue(&self, job: T) {
        (self.0).0.push(job)
    }
}

/// The `DynQueue<T>` which can be parallel iterated over
pub struct DynQueue<'a, T, U: Queue<T>>(Arc<DynQueueInner<'a, T, U>>);

impl<'a, T, U> UnindexedProducer for DynQueue<'a, T, U>
where
    T: Send + Sync,
    U: IntoDynQueue<T, U> + Queue<T> + Send + Sync,
{
    type Item = (DynQueueHandle<'a, T, U>, T);

    fn split(self) -> (Self, Option<Self>) {
        let len = (self.0).0.len();

        if len >= 2 {
            let new_q = (self.0).0.split_off(len / 2);
            (self, Some(new_q.into_dyn_queue()))
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
            let ret = (self.0).0.pop();

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
    U: IntoDynQueue<T, U> + Queue<T> + Send + Sync,
{
    type Item = (DynQueueHandle<'a, T, U>, T);

    fn drive_unindexed<C>(self, consumer: C) -> <C as Consumer<Self::Item>>::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(self, consumer)
    }
}
