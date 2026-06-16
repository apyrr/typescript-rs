use std::{
    fmt, io,
    sync::{Arc, Mutex},
    time::{Duration, UNIX_EPOCH},
};

use super::{Logger, WriterLogger, format_time};

pub trait LogCollector: Logger + fmt::Display {}

pub struct TestLogCollector {
    logger: WriterLogger,
    builder: Arc<Mutex<Vec<u8>>>,
}

#[derive(Clone)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Logger for TestLogCollector {
    fn error(&self, msg: &[&str]) {
        self.logger.error(msg);
    }

    fn errorf(&self, message: String) {
        self.logger.errorf(message);
    }

    fn warn(&self, msg: &[&str]) {
        self.logger.warn(msg);
    }

    fn warnf(&self, message: String) {
        self.logger.warnf(message);
    }

    fn info(&self, msg: &[&str]) {
        self.logger.info(msg);
    }

    fn infof(&self, message: String) {
        self.logger.infof(message);
    }

    fn log(&self, msg: &[&str]) {
        self.logger.log(msg);
    }

    fn logf(&self, message: String) {
        self.logger.logf(message);
    }

    fn verbose(&self) -> bool {
        self.logger.verbose()
    }

    fn is_verbose(&self) -> bool {
        self.logger.is_verbose()
    }

    fn set_verbose(&self, verbose: bool) {
        self.logger.set_verbose(verbose);
    }
}

impl fmt::Display for TestLogCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.builder.lock().unwrap_or_else(|err| err.into_inner());
        let text = std::str::from_utf8(&bytes).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}

impl LogCollector for TestLogCollector {}

pub fn new_test_logger() -> TestLogCollector {
    let builder = Arc::new(Mutex::new(Vec::new()));
    TestLogCollector {
        logger: WriterLogger::with_prefix(SharedBuffer(builder.clone()), || {
            format_time(UNIX_EPOCH + Duration::from_secs(1_349_085_672))
        }),
        builder,
    }
}
