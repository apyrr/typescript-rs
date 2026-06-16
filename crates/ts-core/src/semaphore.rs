use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub trait Semaphore {
    fn acquire(&self) -> Box<dyn FnOnce() + Send + '_>;
    fn try_acquire(&self, cancelled: impl Fn() -> bool) -> (Box<dyn FnOnce() + Send + '_>, bool);
}

pub struct UnlimitedSemaphore;

impl Semaphore for UnlimitedSemaphore {
    fn acquire(&self) -> Box<dyn FnOnce() + Send + '_> {
        Box::new(|| {})
    }

    fn try_acquire(&self, _cancelled: impl Fn() -> bool) -> (Box<dyn FnOnce() + Send + '_>, bool) {
        (Box::new(|| {}), true)
    }
}

pub struct LimitedSemaphore {
    ch: Arc<(Mutex<usize>, Condvar)>,
    max_concurrency: usize,
}

pub fn new_limited_semaphore(max_concurrency: usize) -> LimitedSemaphore {
    if max_concurrency == 0 {
        panic!("maxConcurrency must be positive");
    }
    LimitedSemaphore {
        ch: Arc::new((Mutex::new(0), Condvar::new())),
        max_concurrency,
    }
}

fn limited_semaphore_release(ch: Arc<(Mutex<usize>, Condvar)>) {
    let (lock, cvar) = &*ch;
    let mut count = lock.lock().unwrap_or_else(|err| err.into_inner());
    while *count == 0 {
        count = cvar.wait(count).unwrap_or_else(|err| err.into_inner());
    }
    *count -= 1;
    cvar.notify_one();
}

impl Semaphore for LimitedSemaphore {
    fn acquire(&self) -> Box<dyn FnOnce() + Send + '_> {
        let (lock, cvar) = &*self.ch;
        let mut count = lock.lock().unwrap_or_else(|err| err.into_inner());
        while *count >= self.max_concurrency {
            count = cvar.wait(count).unwrap_or_else(|err| err.into_inner());
        }
        *count += 1;
        let ch = Arc::clone(&self.ch);
        Box::new(move || limited_semaphore_release(ch))
    }

    fn try_acquire(&self, cancelled: impl Fn() -> bool) -> (Box<dyn FnOnce() + Send + '_>, bool) {
        let (lock, cvar) = &*self.ch;
        let mut count = lock.lock().unwrap_or_else(|err| err.into_inner());
        while *count >= self.max_concurrency {
            if cancelled() {
                return (Box::new(|| {}), false);
            }
            let (next_count, _) = cvar
                .wait_timeout(count, Duration::from_millis(10))
                .unwrap_or_else(|err| err.into_inner());
            count = next_count;
        }
        *count += 1;
        let ch = Arc::clone(&self.ch);
        (Box::new(move || limited_semaphore_release(ch)), true)
    }
}
