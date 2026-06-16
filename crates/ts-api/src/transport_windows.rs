#![cfg(windows)]

use std::io::{self, Read, Write};

use interprocess::{
    TryClone,
    local_socket::{
        GenericFilePath, Listener as LocalSocketListener, ListenerOptions,
        Stream as LocalSocketStream, prelude::*,
    },
};

use crate::{Listener, ReadWriteClose};

// newPipeListener creates a Windows named pipe listener.
pub(crate) fn new_pipe_listener(path: &str) -> io::Result<Box<dyn Listener + Send>> {
    let name = path.to_fs_name::<GenericFilePath>()?;
    let listener = ListenerOptions::new().name(name).create_sync()?;
    Ok(Box::new(WindowsPipeListener {
        path: path.to_owned(),
        listener: Some(listener),
    }))
}

// GeneratePipePath returns a platform-appropriate pipe path for the given name.
pub fn generate_pipe_path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

struct WindowsPipeListener {
    path: String,
    listener: Option<LocalSocketListener>,
}

impl Listener for WindowsPipeListener {
    fn accept(&mut self) -> io::Result<Box<dyn ReadWriteClose>> {
        let listener = self.listener.as_ref().ok_or_else(closed_error)?;
        Ok(Box::new(WindowsPipeStream {
            stream: Some(listener.accept()?),
        }))
    }

    fn close(&mut self) -> io::Result<()> {
        self.listener = None;
        Ok(())
    }

    fn addr_string(&self) -> String {
        self.path.clone()
    }
}

struct WindowsPipeStream {
    stream: Option<LocalSocketStream>,
}

impl ReadWriteClose for WindowsPipeStream {
    fn close(&mut self) -> io::Result<()> {
        self.stream = None;
        Ok(())
    }

    fn clone_reader_writer(&self) -> Box<dyn ReadWriteClose> {
        let stream = self
            .stream
            .as_ref()
            .expect("named pipe stream is open before cloning")
            .try_clone()
            .expect("failed to clone named pipe stream");
        Box::new(Self {
            stream: Some(stream),
        })
    }
}

impl Read for WindowsPipeStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stream = self.stream.as_mut().ok_or_else(closed_error)?;
        stream.read(buf)
    }
}

impl Write for WindowsPipeStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stream = self.stream.as_mut().ok_or_else(closed_error)?;
        stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.flush()?;
        }
        Ok(())
    }
}

fn closed_error() -> io::Error {
    io::Error::from(io::ErrorKind::NotConnected)
}
