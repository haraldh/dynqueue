use rayon;
use rayon::iter::plumbing::{
    bridge_unindexed, Consumer, Folder, UnindexedConsumer, UnindexedProducer,
};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub struct JobQueueInner<'a, T> {
    lifo: std::sync::RwLock<Vec<T>>,
    handle_count: AtomicUsize,
    state: PhantomData<&'a ()>,
}

impl<'a, T> JobQueueInner<'a, T> {
    fn new(lifo: Vec<T>) -> Self {
        Self {
            lifo: RwLock::new(lifo),
            handle_count: AtomicUsize::new(0),
            state: PhantomData,
        }
    }

    fn push(&self, job: T) {
        if let Ok(mut t) = self.lifo.write() {
            t.push(job)
        }
    }

    fn done(&self, handle: &mut JobQueueHandle<T>) {
        if let Ok(lock) = self.lifo.write() {
            self.handle_count.fetch_sub(1, Ordering::Relaxed);
            drop(lock);
        }
        handle.0.take();
    }
}
impl<'a, T> Drop for JobQueueInner<'a, T> {
    fn drop(&mut self) {
        if self.handle_count.load(Ordering::Relaxed) != 0 {
            panic!("JobQueueInner handle_count != 0")
        }
    }
}

pub struct JobQueueHandle<'a, T>(Option<Arc<JobQueueInner<'a, T>>>);

impl<'a, T> JobQueueHandle<'a, T> {
    pub fn push(&self, job: T) {
        if let Some(q) = &self.0 {
            q.push(job)
        } else {
            panic!("JobQueueHandle use after done()")
        }
    }
}

impl<'a, T> Drop for JobQueueHandle<'a, T> {
    fn drop(&mut self) {
        if let Some(q) = self.0.take() {
            q.done(self);
        } else {
            panic!("JobQueueHandle has no queue!!")
        }
    }
}

pub struct JobQueue<'a, T>(Arc<JobQueueInner<'a, T>>);

impl<'a, T> JobQueue<'a, T> {
    pub fn new(lifo: Vec<T>) -> Self {
        Self(Arc::new(JobQueueInner::new(lifo)))
    }
}

impl<'a, T> UnindexedProducer for JobQueue<'a, T>
where
    T: Send + Sync + 'a,
{
    type Item = (JobQueueHandle<'a, T>, T);

    fn split(self) -> (Self, Option<Self>) {
        //eprintln!("{:?}: split start", std::thread::current().id());
        let len = {
            let q = self.0.lifo.read().unwrap();
            q.len()
        };
        if len >= 2 {
            let new_q = {
                let mut q = self.0.lifo.write().unwrap();
                let split_off = q.split_off(len / 2);
                /*eprintln!(
                    "{:?}: split len {}",
                    std::thread::current().id(),
                    split_off.len()
                );*/
                JobQueue::new(split_off)
            };
            //eprintln!("{:?}: split end", std::thread::current().id());
            (self, Some(new_q))
        } else {
            //eprintln!("{:?}: split none", std::thread::current().id());
            (self, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        let mut folder = folder;
        let mut count: usize;
        loop {
            //eprintln!("{:?}: l1", std::thread::current().id());
            let (handle, ret) = {
                let mut q = self.0.lifo.write().unwrap();
                count = self.0.handle_count.fetch_add(1, Ordering::Relaxed);
                (JobQueueHandle(Some(self.0.clone())), q.pop())
            };
            //eprintln!("{:?}: l2", std::thread::current().id());

            if let Some(v) = ret {
                //eprintln!("{:?}: consume", std::thread::current().id());
                folder = folder.consume((handle, v));

                if folder.full() {
                    //eprintln!("{:?}: full", std::thread::current().id());
                    break;
                }
            } else {
                if count != 0 {
                    //eprintln!("{:?}: Waiting for handles", std::thread::current().id());
                    std::thread::sleep(Duration::from_millis((1) as _));
                    continue;
                }
                //eprintln!("{:?}: count==0", std::thread::current().id());
                break;
            }
        }
        folder
    }
}

impl<'a, T> rayon::iter::ParallelIterator for JobQueue<'a, T>
where
    T: Send + Sync + 'a,
{
    type Item = (JobQueueHandle<'a, T>, T);

    fn drive_unindexed<C>(self, consumer: C) -> <C as Consumer<Self::Item>>::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(self, consumer)
    }
}

#[cfg(test)]
mod tests {
    use crate::JobQueue;
    const SLEEP_MS: u64 = 10;

    #[test]
    fn jobqueue_iter_test() {
        use rayon::iter::IntoParallelIterator as _;
        use rayon::iter::ParallelIterator as _;
        use std::time::Duration;
        /*
              rayon::ThreadPoolBuilder::new()
                  .num_threads(100)
                  .build_global()
                  .unwrap();
        */
        eprintln!();
        let mut q = Vec::<usize>::new();
        q.push(1);
        q.push(2);
        q.push(3);
        q.push(4);
        q.push(5);
        q.push(6);
        q.push(7);
        q.push(8);
        let jq = JobQueue::new(q);
        let now = std::time::Instant::now();

        let mut res = jq
            .into_par_iter()
            .map(|(h, v)| {
                if v % 2 == 0 {
                    h.push(5);
                }
                if v % 3 == 0 {
                    h.push(5);
                }
                if v % 4 == 0 {
                    h.push(5);
                }
                eprintln!("{}", v);
                std::thread::sleep(Duration::from_millis(SLEEP_MS));
                v
            })
            .collect::<Vec<_>>();
        eprintln!("jobqueue: elapsed = {:#?}", now.elapsed());
        res.sort();
        assert_eq!(res, vec![1, 2, 3, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 7, 8]);
        eprintln!(
            "jobqueue: instead of = {}ms",
            res.iter().len() * SLEEP_MS as usize
        );
    }
    #[test]
    fn par_iter_test() {
        use rayon::iter::IntoParallelIterator as _;
        use rayon::iter::ParallelIterator as _;
        use std::time::Duration;

        let now = std::time::Instant::now();

        let res = vec![1, 2, 3, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 6, 7, 8]
            .into_par_iter()
            .map(|v| {
                eprintln!("{}", v);
                std::thread::sleep(Duration::from_millis(SLEEP_MS));
                v
            })
            .collect::<Vec<_>>();
        eprintln!("par_iter: elapsed = {:#?}", now.elapsed());
        eprintln!("par_iter: instead of = {}ms", res.iter().len() * 10);
    }
}
