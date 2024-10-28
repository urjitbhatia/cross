use crossbeam_deque::Worker;
use std::error::Error;

/// Result type that can contain an optional value or an error
pub type JobResult<T, E> = Result<Option<T>, E>;

/// A job that can be processed by a worker, can be implemented by a closure as well
pub trait GraphJob<IN, OUT, E>: Clone + Send
where
    E: Error + Send,
{
    /// Process a task, returning either Some(value), None, or an Error
    fn process(&self, input: IN, worker: &Worker<IN>) -> JobResult<OUT, E>;
}

// Implement the trait for Fn types that match the signature
impl<IN, OUT, E, F> GraphJob<IN, OUT, E> for F
where
    F: Fn(IN, &Worker<IN>) -> JobResult<OUT, E> + Clone + Send,
    E: Error + Send,
{
    fn process(&self, input: IN, worker: &Worker<IN>) -> JobResult<OUT, E> {
        self(input, worker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    // Custom error type for testing
    #[derive(Debug, PartialEq)]
    struct TestError(String);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "TestError: {}", self.0)
        }
    }

    impl Error for TestError {}

    // Custom job struct for testing
    #[derive(Clone)]
    struct TestJob {
        multiplier: i32,
    }

    impl GraphJob<i32, i32, TestError> for TestJob {
        fn process(&self, input: i32, _worker: &Worker<i32>) -> JobResult<i32, TestError> {
            if input < 0 {
                Err(TestError("negative input".to_string()))
            } else if input == 0 {
                Ok(None)
            } else {
                Ok(Some(input * self.multiplier))
            }
        }
    }

    #[test]
    fn test_job_struct() {
        let worker = Worker::new_fifo();
        let job = TestJob { multiplier: 2 };

        // Test successful case
        assert_eq!(job.process(5, &worker), Ok(Some(10)));

        // Test None case
        assert_eq!(job.process(0, &worker), Ok(None));

        // Test error case
        assert_eq!(
            job.process(-1, &worker),
            Err(TestError("negative input".to_string()))
        );
    }

    #[test]
    fn test_job_closure() {
        let worker = Worker::new_fifo();
        let job = |x: i32, _w: &Worker<i32>| -> JobResult<i32, TestError> {
            if x < 0 {
                Err(TestError("negative input".to_string()))
            } else if x == 0 {
                Ok(None)
            } else {
                Ok(Some(x * 3))
            }
        };

        // Test successful case
        assert_eq!(job(5, &worker), Ok(Some(15)));

        // Test None case
        assert_eq!(job(0, &worker), Ok(None));

        // Test error case
        assert_eq!(
            job(-1, &worker),
            Err(TestError("negative input".to_string()))
        );
    }
}
