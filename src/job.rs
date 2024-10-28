use std::error::Error;

pub struct Job<T>(pub String, pub Vec<T>);

impl<T> Job<T> {
    pub fn new(name: String, args: Vec<T>) -> Self {
        Self(name, args)
    }

    pub fn run<R>(
        &self,
        run_fn: fn(&Self) -> Result<R, Box<dyn Error>>,
    ) -> Result<R, Box<dyn Error>> {
        println!("Running job: {}", self.0);
        run_fn(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiply_numbers() -> Result<(), Box<dyn Error>> {
        let job = Job::new("multiply_numbers".to_string(), vec![2, 3]);
        let result = job.run(|job| {
            let result = job.1.iter().product::<i32>();
            println!("result: {}", result);
            Ok(result)
        });
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.unwrap(), 6);
        Ok(())
    }
}
