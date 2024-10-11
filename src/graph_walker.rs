use crossbeam_deque::{Injector, Stealer, Worker};
use std::sync::Barrier;
use std::{
    iter,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

// find_task fetches the next available task
fn find_task<T>(local: &Worker<T>, global: &Injector<T>, stealers: &[Stealer<T>]) -> Option<T> {
    // Pop a task from the local queue, if not empty.
    local.pop().or_else(|| {
        // Otherwise, we need to look for a task elsewhere.
        iter::repeat_with(|| {
            // Try stealing a batch of tasks from the global queue.
            global
                .steal_batch_and_pop(local)
                // Or try stealing a task from one of the other threads.
                .or_else(|| stealers.iter().map(|s| s.steal()).collect())
        })
        // Loop while no task was stolen and any steal operation needs to be retried.
        .find(|s| !s.is_retry())
        // Extract the stolen task, if there is one.
        .and_then(|s| s.success())
    })
}

pub fn walk<IN, OUT, JOB>(initial: Vec<IN>, num_workers: usize, job: JOB) -> Vec<OUT>
where
    IN: Send,
    OUT: Send,
    JOB: Fn(IN, &Worker<IN>) -> Option<OUT> + Clone + Send,
{
    // Create crossbeam_deque injector/worker/stealers
    let injector = Injector::new();
    let workers: Vec<_> = (0..num_workers).map(|_| Worker::new_fifo()).collect();
    let stealers: Vec<_> = workers.iter().map(|w| w.stealer()).collect();
    let active_counter = ActiveCounter::new();
    let started_counter = ActiveCounter::new();
    let num_workers = workers.len();
    let barrier = Arc::new(Barrier::new(num_workers));

    // Seed injector with initial data
    for item in initial.into_iter() {
        injector.push(item);
    }

    // Create single scope to contain all workers
    let result: Vec<OUT> = crossbeam_utils::thread::scope(|scope| {
        println!(
            "Top level thread: {:?} Num workers: {:?}",
            thread::current().id(),
            workers.len()
        );

        // Container for all workers
        let mut worker_scopes: Vec<_> = Default::default();

        // Create each worker
        for worker in workers.into_iter() {
            // Make copy of data so we can move clones or references into closure
            let injector_borrow = &injector;
            let stealers_copy = stealers.clone();
            let job_copy = job.clone();
            let mut counter_copy = active_counter.clone();
            let mut started_counter_copy = started_counter.clone();
            let barrier = Arc::clone(&barrier);

            // Create scope for single worker
            let s = scope.spawn(move |_| {
                println!("Creating thread: {:?}", thread::current().id());

                // backoff spinner for sleeping
                let backoff = crossbeam_utils::Backoff::new();
                // {
                //     // wait for all threads to start
                //     let at = started_counter_copy.take_token();
                //     loop {
                //         let cc = started_counter_copy.current_count();
                //         if cc >= num_workers {
                //             break;
                //         }
                //         println!("Thread: {:?} current count: {cc}", thread::current().id());
                //         thread::sleep(Duration::from_millis(500));
                //         backoff.snooze();
                //     }
                //     drop(at);
                // }

                // results of this worker
                let mut worker_results: Vec<_> = Default::default();

                // Wait for all threads to get initialized
                barrier.wait();

                // Loop until all workers idle
                loop {
                    {
                        let tok = counter_copy.take_token();
                        // look for work
                        while let Some(item) = find_task(&worker, injector_borrow, &stealers_copy) {
                            backoff.reset();

                            // do work
                            if let Some(result) = job_copy(item, &worker) {
                                worker_results.push(result);
                            }
                        }
                        drop(tok)
                    };

                    // thread::sleep(std::time::Duration::from_secs(5));
                    // no work, check if all workers are idle
                    if counter_copy.is_zero() {
                        println!("thread: {:?} counter copy was zero", thread::current().id());
                        break;
                    }

                    // sleep
                    // thread::sleep(Duration::from_millis(500))
                    backoff.snooze();
                }
                println!("Finished thread: {:?}", thread::current().id());
                // Results for this worker
                worker_results
            });

            worker_scopes.push(s);
        }
        println!("Total number of worker scopes: {:?}", worker_scopes.len());

        // run all workers to completion and combine their results
        worker_scopes
            .into_iter()
            .filter_map(|s| s.join().ok())
            .flatten()
            .collect()
    })
    .unwrap();

    result
}

// used for testing the graph walker
pub fn _run() {
    walk(vec![1], 2, |f, w| {
        println!("walk {f} in thread: {:?}", thread::current().id());
        if f < 500 {
            w.push(f + 1);
        }
        thread::sleep(std::time::Duration::from_secs(180));
        Some(1)
    });
}

// Helpers to track when all workers are done
#[derive(Clone)]
struct ActiveCounter {
    active_count: Arc<AtomicUsize>,
}

impl ActiveCounter {
    pub fn take_token(&mut self) -> ActiveToken {
        self.active_count.fetch_add(1, Ordering::SeqCst);
        ActiveToken {
            active_count: self.active_count.clone(),
        }
    }

    pub fn new() -> ActiveCounter {
        ActiveCounter {
            active_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.active_count.load(Ordering::SeqCst) == 0
    }

    pub fn current_count(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }
}

struct ActiveToken {
    active_count: Arc<AtomicUsize>,
}

impl ActiveToken {
    fn is_zero(&mut self) -> bool {
        self.active_count.load(Ordering::SeqCst) == 0
    }
}

impl Drop for ActiveToken {
    fn drop(&mut self) {
        self.active_count.fetch_sub(1, Ordering::SeqCst);
    }
}
