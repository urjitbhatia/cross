use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

// Helpers to track when all workers are done
#[derive(Clone)]
pub struct ActiveCounter {
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

    #[allow(dead_code)]
    pub fn current_count(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }
}

pub struct ActiveToken {
    active_count: Arc<AtomicUsize>,
}

impl ActiveToken {
    #[allow(dead_code)]
    fn is_zero(&mut self) -> bool {
        self.active_count.load(Ordering::SeqCst) == 0
    }
}

impl Drop for ActiveToken {
    fn drop(&mut self) {
        self.active_count.fetch_sub(1, Ordering::SeqCst);
    }
}
