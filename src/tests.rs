use crate::{DynQueueHandle, IntoDynQueue, Queue};
use std::collections::VecDeque;
use std::sync::Mutex;

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


#[test]
fn dynqueue_iter_test_parallelism() {
    use rayon::current_num_threads;
    use std::time::{Duration, Instant};

    let threads = current_num_threads();
    // With only one worker there is nothing to parallelize.
    if threads <= 1 {
        return;
    }

    // Lots of fixed-sleep items: enough that sequential execution clearly
    // blows past the budget while a genuinely parallel run finishes well under it.
    let jobs = 64u32;
    let per = Duration::from_millis(50);
    let sequential = jobs * per; // 64 * 50ms = 3.2 s

    let start = Instant::now();
    (0..jobs)
        .collect::<Vec<_>>()
        .into_dyn_queue()
        .for_each_dyn(|_, _| {
            std::thread::sleep(per);
        });
    let elapsed = start.elapsed();

    // Regression guard: the original implementation kept all dynamically
    // enqueued work on the producing thread (effectively sequential, ~= sequential).
    // Parallel execution (>= 2 threads) must finish well below that. 0.6x leaves
    // a generous cushion for scheduling overhead while still separating 3.2 s
    // (sequential) from <= 1.6 s (parallel).
    assert!(
        elapsed < sequential.mul_f64(0.6),
        "work was not parallelized: elapsed {elapsed:?} >= 0.6 * sequential {sequential:?}; only {threads} threads were used"
    );
}
