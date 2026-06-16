use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, AtomicU64, Ordering},
    },
    time::SystemTime,
};

use super::{LogCollector, Logger, format_time};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct LogEntry {
    seq: u64,
    time: SystemTime,
    message: String,
    child: Option<Arc<LogTree>>,
}

fn new_log_entry(child: Option<Arc<LogTree>>, message: String) -> LogEntry {
    LogEntry {
        seq: SEQ.fetch_add(1, Ordering::SeqCst) + 1,
        time: SystemTime::now(),
        message,
        child,
    }
}

pub struct LogTree {
    name: String,
    logs: Mutex<Vec<LogEntry>>,
    root: Mutex<Option<Arc<LogTree>>>,
    level: i32,
    verbose: Mutex<bool>,

    // Only set on root
    count: AtomicI32,
    string_length: AtomicI32,
}

pub fn new_log_tree(name: impl Into<String>) -> Arc<LogTree> {
    let lc = Arc::new(LogTree {
        name: name.into(),
        logs: Mutex::new(Vec::new()),
        root: Mutex::new(None),
        level: 0,
        verbose: Mutex::new(false),
        count: AtomicI32::new(0),
        string_length: AtomicI32::new(0),
    });
    *lc.root.lock().unwrap_or_else(|err| err.into_inner()) = Some(lc.clone());
    lc
}

impl LogTree {
    fn root(&self) -> Arc<LogTree> {
        self.root
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("LogTree root is not set"))
    }

    fn add(&self, log: LogEntry) {
        // indent + header + message + newline
        let root = self.root();
        root.string_length.fetch_add(
            self.level + 15 + log.message.len() as i32 + 1,
            Ordering::SeqCst,
        );
        root.count.fetch_add(1, Ordering::SeqCst);
        self.logs
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(log);
    }

    pub fn embed(&self, logs: Arc<LogTree>) {
        let count = logs.count.load(Ordering::SeqCst);
        let root = self.root();
        root.string_length.fetch_add(
            logs.string_length.load(Ordering::SeqCst) + count * self.level,
            Ordering::SeqCst,
        );
        root.count.fetch_add(count, Ordering::SeqCst);
        let log = new_log_entry(Some(logs.clone()), logs.name.clone());
        self.add(log);
    }

    pub fn fork(&self, message: impl Into<String>) -> Arc<LogTree> {
        let child = Arc::new(LogTree {
            name: String::new(),
            logs: Mutex::new(Vec::new()),
            root: Mutex::new(Some(self.root())),
            level: self.level + 1,
            verbose: Mutex::new(*self.verbose.lock().unwrap_or_else(|err| err.into_inner())),
            count: AtomicI32::new(0),
            string_length: AtomicI32::new(0),
        });
        let log = new_log_entry(Some(child.clone()), message.into());
        self.add(log);
        child
    }

    fn write_logs_recursive(&self, builder: &mut String, indent: &str) {
        for log in self
            .logs
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
        {
            builder.push_str(indent);
            builder.push_str(&format_time(log.time));
            builder.push(' ');
            builder.push_str(&log.message);
            builder.push('\n');
            let _ = log.seq;
            if let Some(child) = &log.child {
                child.write_logs_recursive(builder, &format!("{indent}\t"));
            }
        }
    }
}

impl Logger for LogTree {
    fn log(&self, message: &[&str]) {
        let log = new_log_entry(None, message.concat());
        self.add(log);
    }

    fn logf(&self, message: String) {
        let log = new_log_entry(None, message);
        self.add(log);
    }

    fn is_verbose(&self) -> bool {
        *self.verbose.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn set_verbose(&self, verbose: bool) {
        *self.verbose.lock().unwrap_or_else(|err| err.into_inner()) = verbose;
    }

    fn verbose(&self) -> bool {
        self.is_verbose()
    }

    fn error(&self, msg: &[&str]) {
        self.log(msg);
    }

    fn errorf(&self, message: String) {
        self.logf(message);
    }

    fn warn(&self, msg: &[&str]) {
        self.log(msg);
    }

    fn warnf(&self, message: String) {
        self.logf(message);
    }

    fn info(&self, msg: &[&str]) {
        self.log(msg);
    }

    fn infof(&self, message: String) {
        self.logf(message);
    }
}

impl fmt::Display for LogTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let root = self.root();
        if !std::ptr::eq(root.as_ref(), self) {
            panic!("can only call String on root LogTree");
        }
        let mut builder = String::with_capacity(
            self.string_length.load(Ordering::SeqCst).max(0) as usize + self.name.len() + 20,
        );
        builder.push_str(&format!("======== {} ========\n", self.name));
        self.write_logs_recursive(&mut builder, "");
        f.write_str(&builder)
    }
}

impl LogCollector for LogTree {}
