use std::{
    fmt::{self, Display, Write as _},
    io::Write as _,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{lsproto, server::Server};

use ts_project::Logger as ProjectLogger;

pub struct Logger {
    init_started: Option<Arc<AtomicBool>>,
    outgoing_queue: Option<Arc<Mutex<Vec<lsproto::Message>>>>,
    stderr: Arc<Mutex<Box<dyn std::io::Write + Send + Sync>>>,
    mu: Arc<Mutex<bool>>,
}

pub fn new_logger(server: Arc<Server>) -> Logger {
    Logger {
        stderr: server.stderr.clone(),
        init_started: Some(server.init_started.clone()),
        outgoing_queue: Some(server.outgoing_queue.clone()),
        mu: Arc::new(Mutex::new(false)),
    }
}

pub(crate) fn new_logger_with_stderr(
    stderr: Arc<Mutex<Box<dyn std::io::Write + Send + Sync>>>,
) -> Logger {
    new_logger_with_parts(stderr, None, None)
}

pub(crate) fn new_logger_with_parts(
    stderr: Arc<Mutex<Box<dyn std::io::Write + Send + Sync>>>,
    init_started: Option<Arc<AtomicBool>>,
    outgoing_queue: Option<Arc<Mutex<Vec<lsproto::Message>>>>,
) -> Logger {
    Logger {
        init_started,
        outgoing_queue,
        stderr,
        mu: Arc::new(Mutex::new(false)),
    }
}

impl Clone for Logger {
    fn clone(&self) -> Self {
        Self {
            init_started: self.init_started.clone(),
            outgoing_queue: self.outgoing_queue.clone(),
            stderr: self.stderr.clone(),
            mu: self.mu.clone(),
        }
    }
}

impl Logger {
    pub fn send_log_message(&self, msg_type: lsproto::MessageType, message: String) {
        if self
            .init_started
            .as_ref()
            .is_some_and(|init_started| init_started.load(Ordering::SeqCst))
        {
            if let Some(outgoing_queue) = &self.outgoing_queue {
                let notification = lsproto::WindowLogMessageInfo.new_notification_message(
                    lsproto::LogMessageParams {
                        r#type: msg_type,
                        message: message.clone(),
                    },
                );
                outgoing_queue
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(notification.message());
                return;
            }
        }

        let mut stderr = self.stderr.lock().unwrap_or_else(|err| err.into_inner());
        let _ = writeln!(stderr, "{message}");
    }

    pub fn log<T>(&self, msg: &[T])
    where
        T: Display,
    {
        self.send_log_message(lsproto::MessageType::Log, sprint(msg));
    }

    pub fn logf(&self, message: impl Display) {
        self.send_log_message(lsproto::MessageType::Log, message.to_string());
    }

    pub fn verbose(&self) -> Option<&Self> {
        if !*self.mu.lock().unwrap_or_else(|err| err.into_inner()) {
            return None;
        }
        Some(self)
    }

    pub fn is_verbose(&self) -> bool {
        *self.mu.lock().unwrap_or_else(|err| err.into_inner())
    }

    pub fn set_verbose(&self, verbose: bool) {
        *self.mu.lock().unwrap_or_else(|err| err.into_inner()) = verbose;
    }

    pub fn error<T>(&self, msg: &[T])
    where
        T: Display,
    {
        self.send_log_message(lsproto::MessageType::Error, sprint(msg));
    }

    pub fn errorf(&self, message: impl Display) {
        self.send_log_message(lsproto::MessageType::Error, message.to_string());
    }

    pub fn warn<T>(&self, msg: &[T])
    where
        T: Display,
    {
        self.send_log_message(lsproto::MessageType::Warning, sprint(msg));
    }

    pub fn warnf(&self, message: impl Display) {
        self.send_log_message(lsproto::MessageType::Warning, message.to_string());
    }

    pub fn info<T>(&self, msg: &[T])
    where
        T: Display,
    {
        self.send_log_message(lsproto::MessageType::Info, sprint(msg));
    }

    pub fn infof(&self, message: impl Display) {
        self.send_log_message(lsproto::MessageType::Info, message.to_string());
    }
}

impl ProjectLogger for Logger {
    fn log(&self, msg: &[&str]) {
        self.send_log_message(lsproto::MessageType::Log, sprint(msg));
    }

    fn logf(&self, message: String) {
        self.send_log_message(lsproto::MessageType::Log, message);
    }

    fn verbose(&self) -> bool {
        self.is_verbose()
    }

    fn is_verbose(&self) -> bool {
        *self.mu.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn set_verbose(&self, verbose: bool) {
        *self.mu.lock().unwrap_or_else(|err| err.into_inner()) = verbose;
    }

    fn error(&self, msg: &[&str]) {
        self.send_log_message(lsproto::MessageType::Error, sprint(msg));
    }

    fn errorf(&self, message: String) {
        self.send_log_message(lsproto::MessageType::Error, message);
    }

    fn warn(&self, msg: &[&str]) {
        self.send_log_message(lsproto::MessageType::Warning, sprint(msg));
    }

    fn warnf(&self, message: String) {
        self.send_log_message(lsproto::MessageType::Warning, message);
    }

    fn info(&self, msg: &[&str]) {
        self.send_log_message(lsproto::MessageType::Info, sprint(msg));
    }

    fn infof(&self, message: String) {
        self.send_log_message(lsproto::MessageType::Info, message);
    }
}

fn sprint<T>(msg: &[T]) -> String
where
    T: Display,
{
    let mut result = String::new();
    for value in msg {
        let _ = write!(result, "{value}");
    }
    result
}

impl fmt::Debug for Logger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Logger")
            .field("stderr", &"stderr")
            .field(
                "verbose",
                &self.mu.lock().unwrap_or_else(|err| err.into_inner()),
            )
            .finish()
    }
}
