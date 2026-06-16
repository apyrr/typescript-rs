use std::sync::{Arc, Condvar, Mutex};

use ts_core as core;

#[cfg(test)]
mod queue_test;

// Queue manages background tasks execution
#[derive(Clone, Default)]
pub struct Queue {
    state: Arc<(Mutex<State>, Condvar)>,
}

#[derive(Default)]
struct State {
    active: usize,
    closed: bool,
}

// NewQueue creates a new background queue for managing background tasks execution.
pub fn new_queue() -> Queue {
    Queue::default()
}

impl Queue {
    pub fn enqueue(&self, ctx: core::Context, f: impl FnOnce(core::Context)) {
        let (lock, cvar) = &*self.state;
        {
            let mut state = lock.lock().unwrap_or_else(|err| err.into_inner());
            if state.closed {
                return;
            }
            if ctx.err().is_some() {
                return;
            }
            state.active += 1;
        }

        if ctx.err().is_none() {
            f(ctx);
        }
        let mut state = lock.lock().unwrap_or_else(|err| err.into_inner());
        state.active -= 1;
        cvar.notify_all();
    }

    // Wait waits for all active tasks to complete.
    // It does not prevent new tasks from being enqueued while waiting.
    pub fn wait(&self) {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().unwrap_or_else(|err| err.into_inner());
        while state.active != 0 {
            state = cvar.wait(state).unwrap_or_else(|err| err.into_inner());
        }
    }

    pub fn close(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().unwrap_or_else(|err| err.into_inner());
        state.closed = true;
    }
}
