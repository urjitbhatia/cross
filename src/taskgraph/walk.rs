use crate::taskgraph::job::GraphJob;
use crate::taskgraph::task::ActiveCounter;

use crossbeam_deque::{Injector, Stealer, Worker};
use std::fmt::Error;
use std::sync::Barrier;
use std::{iter, sync::Arc, thread};

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

/// Walks a task graph in parallel using work stealing
///
/// This function takes an initial set of tasks and processes them in parallel using multiple worker threads.
/// Each worker can generate new tasks during processing, which are then distributed among all workers.
/// Work stealing is used to balance the load between threads.
///
/// # Arguments
///
/// * `initial` - A vector of initial tasks to process
/// * `num_workers` - Number of worker threads to spawn
/// * `job` - The function that processes each task. Takes a task and a worker queue as arguments.
///          Can optionally return a result and/or generate new tasks by pushing to the worker queue.
///
/// # Type Parameters
///
/// * `IN` - The input task type that must implement Send
/// * `OUT` - The output type that must implement Send
/// * `JOB` - The job function type that must be Clone + Send and take IN and return Option<OUT>
///
/// # Returns
///
/// A vector containing all non-None results produced by the job function

pub fn walk<IN, OUT, JOB>(initial: Vec<IN>, num_workers: usize, job: JOB) -> Vec<OUT>
where
    IN: Send,
    OUT: Send,
    JOB: GraphJob<IN, OUT, Error>,
{
    // Create crossbeam_deque injector/worker/stealers
    let injector = Injector::new();
    // Create num_workers workers
    let workers: Vec<_> = (0..num_workers).map(|_| Worker::new_fifo()).collect();

    // Create task stealers for each worker
    let stealers: Vec<_> = workers.iter().map(|w| w.stealer()).collect();
    // Create active counter to track when all workers are done
    let active_counter = ActiveCounter::new();
    // let started_counter = ActiveCounter::new();

    // Create barrier to wait for all workers to start
    let barrier = Arc::new(Barrier::new(num_workers));

    // Seed injector with initial data
    for item in initial.into_iter() {
        injector.push(item);
    }

    // Create single scope to contain all workers
    let result: Vec<OUT> = crossbeam_utils::thread::scope(|scope| {
        println!(
            "Top level thread id: {:?} Num workers: {:?}",
            thread::current().id(),
            workers.len()
        );

        // Container for all workers
        let mut worker_scopes: Vec<_> = Default::default();

        // Start all the workers
        for worker in workers.into_iter() {
            // Make copy of data so we can move clones or references into closure
            let injector_borrow = &injector;
            let stealers_copy = stealers.clone();
            let job_copy = job.clone();
            let mut counter_copy = active_counter.clone();

            // let mut started_counter_copy = started_counter.clone();

            // No worker will start until the barrier is cleared
            let barrier = Arc::clone(&barrier);

            // Create scope for single worker
            let s = scope.spawn(move |_| {
                println!("Creating worker thread: {:?}", thread::current().id());

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
                            if let Ok(Some(result)) = job_copy.process(item, &worker) {
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn test_empty_input() {
        let job = |_: i32, _: &Worker<i32>| Ok(Some(1));
        let result: Vec<i32> = walk(vec![], 2, job);
        assert!(result.is_empty());
    }

    #[test]
    fn test_simple_processing() {
        let job = |x: i32, _: &Worker<i32>| Ok(Some(x * 2));
        let result: Vec<i32> = walk(vec![1, 2, 3], 2, job);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&2));
        assert!(result.contains(&4));
        assert!(result.contains(&6));
    }

    #[test]
    fn test_task_generation() {
        // Keep track of processed numbers
        static PROCESSED: AtomicUsize = AtomicUsize::new(0);

        let job = |x: i32, w: &Worker<i32>| {
            PROCESSED.fetch_add(1, Ordering::SeqCst);

            // Generate two new tasks for numbers less than 3
            if x < 3 {
                w.push(x * 2);
                w.push(x * 2 + 1);
            }

            Ok(Some(x))
        };

        let result: Vec<i32> = walk(vec![1], 3, job);

        // Should process: 1 -> [2,3] -> [4,5] (no more tasks for 3,4,5)
        assert_eq!(PROCESSED.load(Ordering::SeqCst), 5);
        assert_eq!(result.len(), 5);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(result.contains(&3));
        assert!(result.contains(&4));
        assert!(result.contains(&5));
    }

    #[test]
    fn test_parallel_execution() {
        static THREADS_USED: AtomicUsize = AtomicUsize::new(0);

        let job = |x: i32, _: &Worker<i32>| {
            // Record this thread
            THREADS_USED.fetch_add(1, Ordering::SeqCst);
            // Simulate work
            thread::sleep(Duration::from_millis(50));
            Ok(Some(x))
        };

        // Process enough items to ensure parallel execution
        let input: Vec<i32> = (0..10).collect();
        let num_workers = 4;
        let result = walk(input, num_workers, job);

        assert_eq!(result.len(), 10);
        // Verify that multiple threads were used
        assert!(THREADS_USED.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn test_none_results() {
        let job = |x: i32, _: &Worker<i32>| {
            // Only return Some for even numbers
            if x % 2 == 0 {
                Ok(Some(x))
            } else {
                Ok(None)
            }
        };

        let result: Vec<i32> = walk(vec![1, 2, 3, 4, 5, 6], 2, job);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&2));
        assert!(result.contains(&4));
        assert!(result.contains(&6));
    }
}
