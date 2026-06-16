use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

pub const CANCELED: &str = "context canceled";

#[derive(Debug, Default)]
struct ContextState {
    canceled: AtomicBool,
    parent: Option<Arc<ContextState>>,
}

impl ContextState {
    fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::SeqCst)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.is_canceled())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Context {
    state: Arc<ContextState>,
    request_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CancelFunc {
    state: Arc<ContextState>,
}

impl Context {
    pub fn background() -> Self {
        Self::default()
    }

    pub fn todo() -> Self {
        Self::background()
    }

    pub fn err(&self) -> Option<String> {
        self.state.is_canceled().then(|| CANCELED.to_string())
    }
}

impl CancelFunc {
    pub fn cancel(&self) {
        self.state.canceled.store(true, Ordering::SeqCst);
    }
}

pub fn with_cancel(ctx: Context) -> (Context, CancelFunc) {
    let state = Arc::new(ContextState {
        canceled: AtomicBool::new(false),
        parent: Some(ctx.state),
    });
    let cancel = CancelFunc {
        state: Arc::clone(&state),
    };
    (
        Context {
            state,
            request_id: ctx.request_id,
        },
        cancel,
    )
}

pub fn background() -> Context {
    Context::background()
}

pub fn with_request_id(mut ctx: Context, id: String) -> Context {
    ctx.request_id = Some(id);
    ctx
}

pub fn get_request_id(ctx: &Context) -> String {
    ctx.request_id.clone().unwrap_or_default()
}

pub fn sleep_or_done(duration: Duration, ctx: &Context) -> bool {
    let interval = Duration::from_millis(10);
    let deadline = std::time::Instant::now() + duration;
    loop {
        if ctx.err().is_some() {
            return false;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return true;
        }
        thread::sleep((deadline - now).min(interval));
    }
}

#[derive(Debug)]
pub struct Timer {
    cancel: CancelFunc,
}

impl Timer {
    pub fn stop(self) {
        self.cancel.cancel();
    }
}

pub fn after_func(duration: Duration, f: impl FnOnce() + Send + 'static) -> Timer {
    let (ctx, cancel) = with_cancel(Context::background());
    let timer_cancel = cancel.clone();
    thread::spawn(move || {
        if sleep_or_done(duration, &ctx) {
            f();
        }
        cancel.cancel();
    });
    Timer {
        cancel: timer_cancel,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    Done,
    Elapsed,
}

pub struct Ticker {
    duration: Duration,
}

impl Ticker {
    pub fn select(&mut self, ctx: &Context) -> Tick {
        if sleep_or_done(self.duration, ctx) {
            Tick::Elapsed
        } else {
            Tick::Done
        }
    }
}

pub fn new_ticker(duration: Duration) -> Ticker {
    Ticker { duration }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_context_has_no_error() {
        assert!(Context::background().err().is_none());
    }

    #[test]
    fn todo_context_has_no_error() {
        assert!(Context::todo().err().is_none());
    }

    #[test]
    fn request_id_round_trips_through_context() {
        let ctx = with_request_id(Context::background(), "req-1".to_string());

        assert_eq!(get_request_id(&ctx), "req-1");
    }

    #[test]
    fn missing_request_id_returns_empty_string() {
        assert_eq!(get_request_id(&Context::background()), "");
    }

    #[test]
    fn request_id_overwrites_previous_value() {
        let ctx = with_request_id(Context::background(), "req-1".to_string());
        let ctx = with_request_id(ctx, "req-2".to_string());

        assert_eq!(get_request_id(&ctx), "req-2");
    }

    #[test]
    fn cancel_sets_context_error_for_clones() {
        let (ctx, cancel) = with_cancel(Context::background());
        let clone = ctx.clone();

        assert!(ctx.err().is_none());
        cancel.cancel();

        assert_eq!(ctx.err().as_deref(), Some(CANCELED));
        assert_eq!(clone.err().as_deref(), Some(CANCELED));
    }

    #[test]
    fn parent_cancel_cancels_child_but_child_cancel_does_not_cancel_parent() {
        let (parent, parent_cancel) = with_cancel(Context::background());
        let (child, child_cancel) = with_cancel(parent.clone());

        child_cancel.cancel();
        assert_eq!(child.err().as_deref(), Some(CANCELED));
        assert!(parent.err().is_none());

        let (child, _) = with_cancel(parent.clone());
        parent_cancel.cancel();
        assert_eq!(parent.err().as_deref(), Some(CANCELED));
        assert_eq!(child.err().as_deref(), Some(CANCELED));
    }

    #[test]
    fn with_cancel_preserves_request_id() {
        let ctx = with_request_id(Context::background(), "req-1".to_string());
        let (ctx, _) = with_cancel(ctx);

        assert_eq!(get_request_id(&ctx), "req-1");
    }
}
