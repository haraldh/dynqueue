use crate::{DynQueueHandle, IntoDynQueue, Queue};
use std::collections::VecDeque;
use std::sync::{Mutex, RwLock};

const SLEEP_MS: u64 = 10;

#[inline]
fn handle_collect<U: Queue<u64>>(h: &DynQueueHandle<u64, U>, v: u64, out: &Mutex<Vec<u64>>) {
    if v.is_multiple_of(2) {
        h.enqueue(11);
    }
    if v.is_multiple_of(3) {
        h.enqueue(11);
    }
    if v.is_multiple_of(4) {
        h.enqueue(11);
    }
    if v == 11 {
        h.enqueue(5);
        h.enqueue(17);
    }
    out.lock().unwrap().push(v);
}

#[inline]
fn get_input() -> Vec<u64> {
    vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
    ]
}

#[inline]
fn get_expected() -> Vec<u64> {
    vec![
        1, 2, 3, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 7,
        8, 9, 10, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
        11, 11, 11, 12, 13, 14, 15, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
        17, 17, 17, 17, 17, 17, 17, 17, 17, 18, 19, 20, 21,
    ]
}

fn assert_result(out: Mutex<Vec<u64>>, expected: &[u64]) {
    let mut res = out.into_inner().unwrap();
    res.sort();
    assert_eq!(res, expected);
}

#[test]
fn dynqueue_iter_test_const_sleep() {
    use std::time::{Duration, Instant};
    let expected = get_expected();
    let med = expected.iter().sum::<u64>() / expected.len() as u64;

    let now = Instant::now();
    let out = Mutex::new(Vec::new());
    get_input().into_dyn_queue().for_each_dyn(|h, v| {
        handle_collect(&h, v, &out);
        std::thread::sleep(Duration::from_millis(SLEEP_MS * med));
    });
    eprintln!("elapsed = {:#?}", now.elapsed());
    assert_result(out, &expected);
}

#[test]
fn dynqueue_iter_test_const_sleep_vecdeque() {
    use std::time::{Duration, Instant};
    let expected = get_expected();
    let med = expected.iter().sum::<u64>() / expected.len() as u64;

    let now = Instant::now();
    let out = Mutex::new(Vec::new());
    VecDeque::from(get_input())
        .into_dyn_queue()
        .for_each_dyn(|h, v| {
            handle_collect(&h, v, &out);
            std::thread::sleep(Duration::from_millis(SLEEP_MS * med));
        });
    eprintln!("elapsed = {:#?}", now.elapsed());
    assert_result(out, &expected);
}

#[cfg(feature = "crossbeam-queue")]
#[test]
fn dynqueue_iter_test_const_sleep_segqueue() {
    use crossbeam_queue::SegQueue;
    use std::time::{Duration, Instant};
    let expected = get_expected();
    let med = expected.iter().sum::<u64>() / expected.len() as u64;

    let jq = SegQueue::new();
    get_input().drain(..).for_each(|ele| jq.push(ele));

    let now = Instant::now();
    let out = Mutex::new(Vec::new());
    jq.into_dyn_queue().for_each_dyn(|h, v| {
        handle_collect(&h, v, &out);
        std::thread::sleep(Duration::from_millis(SLEEP_MS * med));
    });
    eprintln!("elapsed = {:#?}", now.elapsed());
    assert_result(out, &expected);
}

#[test]
fn dynqueue_iter_test_sleep_v() {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let out = Mutex::new(Vec::new());
    get_input().into_dyn_queue().for_each_dyn(|h, v| {
        handle_collect(&h, v, &out);
        std::thread::sleep(Duration::from_millis(SLEEP_MS * v));
    });
    eprintln!("elapsed = {:#?}", now.elapsed());
    assert_result(out, &get_expected());
}

#[test]
fn dynqueue_iter_test_sleep_inv_v() {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let out = Mutex::new(Vec::new());
    get_input().into_dyn_queue().for_each_dyn(|h, v| {
        handle_collect(&h, v, &out);
        std::thread::sleep(Duration::from_millis(SLEEP_MS * (22 - v)));
    });
    eprintln!("elapsed = {:#?}", now.elapsed());
    assert_result(out, &get_expected());
}

#[test]
fn empty_queue() {
    let out = Mutex::new(Vec::new());
    Vec::<u64>::new().into_dyn_queue().for_each_dyn(|h, v| {
        handle_collect(&h, v, &out);
    });
    assert!(out.into_inner().unwrap().is_empty());
}

// Covers the `IntoDynQueue` impls that accept an already-wrapped
// `RwLock<Vec<T>>` / `RwLock<VecDeque<T>>` (src/lib.rs), which the `Vec` and
// `VecDeque` tests above do not exercise.
#[test]
fn dynqueue_iter_test_rwlock_vec_backends() {
    let out = Mutex::new(Vec::new());
    RwLock::new(get_input())
        .into_dyn_queue()
        .for_each_dyn(|h, v| handle_collect(&h, v, &out));
    assert_result(out, &get_expected());

    let out = Mutex::new(Vec::new());
    RwLock::new(VecDeque::from(get_input()))
        .into_dyn_queue()
        .for_each_dyn(|h, v| handle_collect(&h, v, &out));
    assert_result(out, &get_expected());
}

// Regression guard for the crate's core feature: `for_each_dyn` must let each
// job add NEW work while the queue is draining, and that dynamically enqueued
// work must be processed in parallel rather than being forced back onto the
// producing thread (the historical bug). Every job enqueues its two children
// (2v+1, 2v+2 down to `max`) while it runs, so each processed value except the
// seed is itself a dynamically added job. Run on a private 4-thread rayon pool
// via `ThreadPool::install`, isolated from the harness's other rayon work
// (which shares the global pool), so peak overlap deterministically reaches
// the full pool size.
#[test]
fn dynqueue_iter_test_parallelism() {
    use rayon::ThreadPoolBuilder;
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    const THREADS: usize = 4;
    let per = Duration::from_millis(10);
    let max = 64u64;

    let active = AtomicU64::new(0);
    let max_active = AtomicU64::new(0);
    let seen = Mutex::new(BTreeSet::new());

    ThreadPoolBuilder::new()
        .num_threads(THREADS)
        .build()
        .unwrap()
        .install(|| {
            // Seed a single item; every job enqueues its two children while
            // draining, so all other jobs arrive dynamically.
            vec![0u64].into_dyn_queue().for_each_dyn(|h, v| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);

                seen.lock().unwrap().insert(v);

                for c in [2 * v + 1, 2 * v + 2] {
                    if c <= max {
                        h.enqueue(c);
                    }
                }

                std::thread::sleep(per);
                active.fetch_sub(1, Ordering::SeqCst);
            });
        });

    // Every dynamically enqueued job was actually picked up and processed
    // (the closure of {0} under the children rule, bounded by `max`).
    let expected: BTreeSet<u64> = {
        let mut set = BTreeSet::new();
        set.insert(0);
        let mut pending = vec![0u64];
        while let Some(v) = pending.pop() {
            for c in [2 * v + 1, 2 * v + 2] {
                if c <= max && set.insert(c) {
                    pending.push(c);
                }
            }
        }
        set
    };
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen, expected, "some dynamically enqueued jobs were lost");

    // Draining ran in parallel and used the whole pool, not just one thread.
    // On an isolated pool the fan-out makes peak overlap deterministically
    // equal to the number of worker threads.
    assert!(
        max_active.load(Ordering::SeqCst) >= THREADS as u64,
        "work was not parallelized: peak concurrent items = {} with {THREADS} threads",
        max_active.load(Ordering::SeqCst)
    );
}
