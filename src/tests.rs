use crate::{DynQueue, DynQueueHandle, Queue};
use std::collections::VecDeque;

const SLEEP_MS: u64 = 10;

#[inline]
fn handle_queue<U: Queue<u64>>(t: (DynQueueHandle<u64, U>, u64)) -> u64 {
    let (h, v) = t;

    if v % 2 == 0 {
        h.enqueue(11);
    }
    if v % 3 == 0 {
        h.enqueue(11);
    }
    if v % 4 == 0 {
        h.enqueue(11);
    }
    if v == 11 {
        h.enqueue(5);
        h.enqueue(17);
    }
    v
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

#[test]
fn dynqueue_iter_test_const_sleep() {
    use rayon::iter::IntoParallelIterator as _;
    use rayon::iter::ParallelIterator as _;
    use std::time::Duration;
    let expected = get_expected();

    let med = expected.iter().sum::<u64>() / expected.iter().count() as u64;

    let jq = DynQueue::new(get_input());
    let now = std::time::Instant::now();

    let mut res = jq
        .into_par_iter()
        .map(handle_queue)
        .map(|v| {
            std::thread::sleep(Duration::from_millis(SLEEP_MS * med));
            v
        })
        .collect::<Vec<_>>();
    eprintln!("elapsed = {:#?}", now.elapsed());
    res.sort();
    assert_eq!(res, expected);
    eprintln!(
        "instead of = {}ms",
        res.iter().count() * med as usize * SLEEP_MS as usize
    );
}

#[cfg(feature = "crossbeam-queue")]
#[test]
fn dynqueue_iter_test_const_sleep_segqueue() {
    use crossbeam_queue::SegQueue;
    use rayon::iter::IntoParallelIterator as _;
    use rayon::iter::ParallelIterator as _;
    use std::time::Duration;
    let expected = get_expected();

    let med = expected.iter().sum::<u64>() / expected.iter().count() as u64;
    let q = SegQueue::new();
    get_input().drain(..).for_each(|ele| q.push(ele));

    let jq = DynQueue::new(q);
    let now = std::time::Instant::now();

    let mut res = jq
        .into_par_iter()
        .map(handle_queue)
        .map(|v| {
            std::thread::sleep(Duration::from_millis(SLEEP_MS * med));
            v
        })
        .collect::<Vec<_>>();
    eprintln!("elapsed = {:#?}", now.elapsed());
    res.sort();
    assert_eq!(res, expected);
    eprintln!(
        "instead of = {}ms",
        res.iter().count() * med as usize * SLEEP_MS as usize
    );
}

#[test]
fn dynqueue_iter_test_const_sleep_vecdeque() {
    use rayon::iter::IntoParallelIterator as _;
    use rayon::iter::ParallelIterator as _;
    use std::time::Duration;
    let expected = get_expected();

    let med = expected.iter().sum::<u64>() / expected.iter().count() as u64;

    let jq = DynQueue::new(VecDeque::from(get_input()));
    let now = std::time::Instant::now();

    let mut res = jq
        .into_par_iter()
        .map(handle_queue)
        .map(|v| {
            std::thread::sleep(Duration::from_millis(SLEEP_MS * med));
            v
        })
        .collect::<Vec<_>>();
    eprintln!("elapsed = {:#?}", now.elapsed());
    res.sort();
    assert_eq!(res, expected);
    eprintln!(
        "instead of = {}ms",
        res.iter().count() * med as usize * SLEEP_MS as usize
    );
}

#[test]
fn dynqueue_iter_test_sleep_v() {
    use rayon::iter::IntoParallelIterator as _;
    use rayon::iter::ParallelIterator as _;
    use std::time::Duration;

    let jq = DynQueue::new(get_input());

    let now = std::time::Instant::now();

    let mut res = jq
        .into_par_iter()
        .map(handle_queue)
        .map(|v| {
            std::thread::sleep(Duration::from_millis(SLEEP_MS * v));
            v
        })
        .collect::<Vec<_>>();
    eprintln!("elapsed = {:#?}", now.elapsed());
    res.sort();
    assert_eq!(res, get_expected());
    eprintln!("instead of = {}ms", res.iter().sum::<u64>() * SLEEP_MS);
}

#[test]
fn dynqueue_iter_test_sleep_inv_v() {
    use rayon::iter::IntoParallelIterator as _;
    use rayon::iter::ParallelIterator as _;
    use std::time::Duration;

    let jq = DynQueue::new(get_input());

    let now = std::time::Instant::now();

    let mut res = jq
        .into_par_iter()
        .map(handle_queue)
        .map(|v| {
            std::thread::sleep(Duration::from_millis(SLEEP_MS * (22 - v)));
            v
        })
        .collect::<Vec<_>>();
    eprintln!("elapsed = {:#?}", now.elapsed());
    res.sort();
    assert_eq!(res, get_expected());
    eprintln!(
        "instead of = {}ms",
        (res.iter().count() as u64 * 22 - res.iter().sum::<u64>()) * SLEEP_MS
    );
}

#[test]
fn par_iter_test() {
    use rayon::iter::IntoParallelIterator as _;
    use rayon::iter::ParallelIterator as _;
    use std::time::Duration;

    let now = std::time::Instant::now();

    let res = get_expected()
        .into_par_iter()
        .map(|v| {
            std::thread::sleep(Duration::from_millis(SLEEP_MS * v as u64));
            v
        })
        .collect::<Vec<_>>();
    eprintln!("elapsed = {:#?}", now.elapsed());
    eprintln!("instead of = {}ms", res.iter().sum::<u64>() * SLEEP_MS);
}
