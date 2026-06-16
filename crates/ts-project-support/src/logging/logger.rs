use std::{
    fmt::Write as _,
    io,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub trait Logger {
    // Error logs an error message.
    fn error(&self, msg: &[&str]);
    // Errorf logs a formatted error message.
    fn errorf(&self, message: String);
    // Warn logs a warning message.
    fn warn(&self, msg: &[&str]);
    // Warnf logs a formatted warning message.
    fn warnf(&self, message: String);
    // Info logs an info message.
    fn info(&self, msg: &[&str]);
    // Infof logs a formatted info message.
    fn infof(&self, message: String);
    // Log prints a line to the output writer with a header.
    fn log(&self, msg: &[&str]);
    // Logf prints a formatted line to the output writer with a header.
    fn logf(&self, message: String);

    // Verbose returns the logger instance if verbose logging is enabled, and otherwise returns nil.
    // A nil logger created with `logging.NewLogger` is safe to call methods on.
    fn verbose(&self) -> bool;
    // IsVerbose returns true if verbose logging is enabled, and false otherwise.
    fn is_verbose(&self) -> bool;
    // SetVerbose sets the verbose logging flag.
    fn set_verbose(&self, verbose: bool);
}

pub struct WriterLogger {
    verbose: Mutex<bool>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    prefix: fn() -> String,
}

pub fn new_logger(writer: impl io::Write + Send + 'static) -> WriterLogger {
    WriterLogger {
        writer: Arc::new(Mutex::new(Box::new(writer))),
        verbose: Mutex::new(false),
        prefix: || format_time(SystemTime::now()),
    }
}

impl WriterLogger {
    pub fn with_prefix(
        writer: impl io::Write + Send + 'static,
        prefix: fn() -> String,
    ) -> WriterLogger {
        WriterLogger {
            writer: Arc::new(Mutex::new(Box::new(writer))),
            verbose: Mutex::new(false),
            prefix,
        }
    }
}

impl Logger for WriterLogger {
    fn log(&self, msg: &[&str]) {
        let mut writer = self.writer.lock().unwrap_or_else(|err| err.into_inner());
        let mut line = String::new();
        let _ = writeln!(line, "{} {}", (self.prefix)(), msg.concat());
        let _ = writer.write_all(line.as_bytes());
    }

    fn logf(&self, message: String) {
        let mut writer = self.writer.lock().unwrap_or_else(|err| err.into_inner());
        let mut line = String::new();
        let _ = writeln!(line, "{} {}", (self.prefix)(), message);
        let _ = writer.write_all(line.as_bytes());
    }

    fn verbose(&self) -> bool {
        self.is_verbose()
    }

    fn is_verbose(&self) -> bool {
        *self.verbose.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn set_verbose(&self, verbose: bool) {
        *self.verbose.lock().unwrap_or_else(|err| err.into_inner()) = verbose;
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

pub fn format_time(t: SystemTime) -> String {
    let duration = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_millis = duration.as_millis() % 86_400_000;
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis / 60_000) % 60;
    let seconds = (total_millis / 1_000) % 60;
    let millis = total_millis % 1_000;
    format!("[{hours:02}:{minutes:02}:{seconds:02}.{millis:03}]")
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use super::{Logger, WriterLogger};

    #[derive(Clone)]
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writer_logger_writes_prefixed_lines_to_writer() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let logger =
            WriterLogger::with_prefix(SharedBuffer(bytes.clone()), || "[12:34:56.789]".to_string());

        logger.log(&["hello", " ", "world"]);
        logger.logf("formatted".to_string());

        let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert_eq!(
            output,
            "[12:34:56.789] hello world\n[12:34:56.789] formatted\n"
        );
    }
}
