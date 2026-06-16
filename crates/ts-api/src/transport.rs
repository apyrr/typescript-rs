use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
};

#[cfg(not(windows))]
use crate::transport_unix::new_pipe_listener;
#[cfg(windows)]
use crate::transport_windows::new_pipe_listener;

// Transport is an interface for accepting connections from API clients.
pub trait Transport {
    // Accept waits for and returns the next connection.
    fn accept(&mut self) -> io::Result<Box<dyn ReadWriteClose>>;
    // Close stops the transport from accepting new connections.
    fn close(&mut self) -> io::Result<()>;
}

pub trait ReadClose: Read + Send {
    fn close(&mut self) -> io::Result<()>;
}

pub trait WriteClose: Write + Send {
    fn close(&mut self) -> io::Result<()>;
}

pub trait ReadWriteClose: Read + Write + Send + Sync {
    fn close(&mut self) -> io::Result<()>;

    fn clone_reader_writer(&self) -> Box<dyn ReadWriteClose> {
        panic!("clone_reader_writer requires a clonable transport connection")
    }
}

impl ReadClose for io::Stdin {
    fn close(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteClose for io::Stdout {
    fn close(&mut self) -> io::Result<()> {
        self.flush()
    }
}

pub trait Listener: Send {
    fn accept(&mut self) -> io::Result<Box<dyn ReadWriteClose>>;
    fn close(&mut self) -> io::Result<()>;
    fn addr_string(&self) -> String;
}

// PipeTransport accepts connections on a Unix domain socket or Windows named pipe.
pub struct PipeTransport {
    listener: Box<dyn Listener + Send>,
}

// NewPipeTransport creates a new transport listening on the given path.
// On Unix, this creates a Unix domain socket. On Windows, this creates a named pipe.
pub fn new_pipe_transport(path: &str) -> io::Result<PipeTransport> {
    let listener = new_pipe_listener(path)?;
    Ok(PipeTransport { listener })
}

// Accept implements Transport.
impl Transport for PipeTransport {
    fn accept(&mut self) -> io::Result<Box<dyn ReadWriteClose>> {
        self.listener.accept()
    }

    // Close implements Transport.
    fn close(&mut self) -> io::Result<()> {
        self.listener.close()
    }
}

impl PipeTransport {
    // Path returns the path of the pipe/socket.
    pub fn path(&self) -> String {
        self.listener.addr_string()
    }
}

// StdioTransport wraps stdin/stdout as a single connection transport.
// It only accepts one connection.
pub struct StdioTransport {
    stdin: Option<Box<dyn ReadClose>>,
    stdout: Option<Box<dyn WriteClose>>,
    used: bool,
}

// NewStdioTransport creates a transport using the given stdin/stdout.
pub fn new_stdio_transport(
    stdin: Box<dyn ReadClose>,
    stdout: Box<dyn WriteClose>,
) -> StdioTransport {
    StdioTransport {
        stdin: Some(stdin),
        stdout: Some(stdout),
        used: false,
    }
}

impl Transport for StdioTransport {
    // Accept implements Transport.
    fn accept(&mut self) -> io::Result<Box<dyn ReadWriteClose>> {
        if self.used {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        self.used = true;
        Ok(Box::new(SharedStdioConn {
            inner: Arc::new(Mutex::new(StdioConn {
                stdin: self
                    .stdin
                    .take()
                    .expect("stdin is present before first accept"),
                stdout: self
                    .stdout
                    .take()
                    .expect("stdout is present before first accept"),
            })),
        }))
    }

    // Close implements Transport.
    fn close(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct StdioConn {
    stdin: Box<dyn ReadClose>,
    stdout: Box<dyn WriteClose>,
}

#[derive(Clone)]
struct SharedStdioConn {
    inner: Arc<Mutex<StdioConn>>,
}

impl Read for SharedStdioConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .stdin
            .read(buf)
    }
}

impl Write for SharedStdioConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .stdout
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .stdout
            .flush()
    }
}

impl ReadWriteClose for SharedStdioConn {
    fn close(&mut self) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let err1 = inner.stdin.close().err();
        let err2 = inner.stdout.close().err();
        if let Some(err1) = err1 {
            return Err(err1);
        }
        if let Some(err2) = err2 {
            return Err(err2);
        }
        Ok(())
    }

    fn clone_reader_writer(&self) -> Box<dyn ReadWriteClose> {
        Box::new(self.clone())
    }
}
