use crate::semaphore::{LimitedSemaphore, Semaphore};

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

pub type WorkError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub trait WorkGroup {
    // Queue queues a function to run. It may be invoked immediately, or deferred until RunAndWait.
    // It is not safe to call Queue after RunAndWait has returned.
    fn queue(&self, f: Box<dyn FnOnce()>);

    // RunAndWait runs all queued functions, blocking until they have all completed.
    fn run_and_wait(&self);
}

pub fn new_work_group(single_threaded: bool) -> Box<dyn WorkGroup> {
    let _ = single_threaded;
    // PORT NOTE: TS-Go runs non-single-threaded workgroups through goroutines.
    // The Rust WorkGroup trait currently accepts non-Send closures because the
    // file-loader, project-reference, and build-task flows still capture
    // non-Send compiler/project state. Spawning those closures would either fail
    // to compile or require unsafe Send/Sync assertions over the AST/checker
    // graph. Keep the upstream queue/run contract single-threaded until the
    // queued work items can be expressed as Send.
    Box::new(SingleThreadedWorkGroup::default())
}

#[derive(Default)]
struct SingleThreadedWorkGroup {
    done: AtomicBool,
    fns: Mutex<Vec<Box<dyn FnOnce()>>>,
}

impl WorkGroup for SingleThreadedWorkGroup {
    fn queue(&self, f: Box<dyn FnOnce()>) {
        if self.done.load(Ordering::SeqCst) {
            panic!("Queue called after RunAndWait returned");
        }

        self.fns
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(f);
    }

    fn run_and_wait(&self) {
        let _done_guard = DoneGuard { done: &self.done };
        loop {
            let f = self.pop();
            let Some(f) = f else {
                return;
            };
            f();
        }
    }
}

impl SingleThreadedWorkGroup {
    fn pop(&self) -> Option<Box<dyn FnOnce()>> {
        self.fns.lock().unwrap_or_else(|err| err.into_inner()).pop()
    }
}

struct DoneGuard<'a> {
    done: &'a AtomicBool,
}

impl Drop for DoneGuard<'_> {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
    }
}

// ThrottleGroup is like errgroup.Group but with global concurrency limiting via a semaphore.
pub struct ThrottleGroup {
    semaphore: Arc<LimitedSemaphore>,
    handles: Mutex<Vec<thread::JoinHandle<Result<(), WorkError>>>>,
}

// NewThrottleGroup creates a new ThrottleGroup with the given context and semaphore for concurrency limiting.
pub fn new_throttle_group(semaphore: Arc<LimitedSemaphore>) -> ThrottleGroup {
    ThrottleGroup {
        semaphore,
        handles: Mutex::new(Vec::new()),
    }
}

impl ThrottleGroup {
    // Go runs the given function in a new goroutine, but first acquires a slot from the semaphore.
    // The semaphore slot is released when the function completes.
    pub fn go(&self, f: impl FnOnce() -> Result<(), WorkError> + Send + 'static) {
        let semaphore = Arc::clone(&self.semaphore);
        let handle = thread::spawn(move || {
            struct Permit<'a> {
                release: Option<Box<dyn FnOnce() + Send + 'a>>,
            }

            impl Drop for Permit<'_> {
                fn drop(&mut self) {
                    if let Some(release) = self.release.take() {
                        release();
                    }
                }
            }

            // Acquire semaphore slot - this will block until a slot is available
            let _permit = Permit {
                release: Some(semaphore.acquire()),
            };

            f()
        });
        self.handles
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(handle);
    }

    // Wait waits for all goroutines to complete and returns the first error encountered, if any.
    pub fn wait(&self) -> Result<(), WorkError> {
        let mut first_error = None;
        let mut handles = self.handles.lock().unwrap_or_else(|err| err.into_inner());
        for handle in handles.drain(..) {
            match handle.join().unwrap() {
                Ok(()) => {}
                Err(err) if first_error.is_none() => first_error = Some(err),
                Err(_) => {}
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn work_group_runs_queued_work() {
        let wg = new_work_group(false);
        let values = Arc::new(Mutex::new(Vec::new()));

        for value in [1, 2, 3] {
            let values = Arc::clone(&values);
            wg.queue(Box::new(move || {
                values
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(value);
            }));
        }

        wg.run_and_wait();

        assert_eq!(
            *values.lock().unwrap_or_else(|err| err.into_inner()),
            vec![3, 2, 1]
        );
    }

    #[test]
    fn work_group_runs_work_queued_during_run() {
        let wg = Arc::new(SingleThreadedWorkGroup::default());
        let values = Arc::new(Mutex::new(Vec::new()));

        let nested_wg = Arc::clone(&wg);
        let nested_values = Arc::clone(&values);
        wg.queue(Box::new(move || {
            nested_values
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(1);
            nested_wg.queue(Box::new({
                let values = Arc::clone(&nested_values);
                move || {
                    values.lock().unwrap_or_else(|err| err.into_inner()).push(2);
                }
            }));
        }));

        wg.run_and_wait();

        assert_eq!(
            *values.lock().unwrap_or_else(|err| err.into_inner()),
            vec![1, 2]
        );
    }

    #[test]
    #[should_panic(expected = "Queue called after RunAndWait returned")]
    fn work_group_rejects_queue_after_run() {
        let wg = new_work_group(true);
        wg.run_and_wait();
        wg.queue(Box::new(|| {}));
    }
}
