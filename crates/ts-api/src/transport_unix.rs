#![cfg(not(windows))]

use std::{
    fs, io,
    os::unix::net::{UnixListener, UnixStream},
    path,
};

use crate::{Listener, ReadWriteClose};

// newPipeListener creates a Unix domain socket listener.
pub(crate) fn new_pipe_listener(path: &str) -> io::Result<Box<dyn Listener + Send>> {
    // Remove any existing socket file
    let _ = fs::remove_file(path);
    Ok(Box::new(UnixPipeListener {
        listener: UnixListener::bind(path)?,
    }))
}

// GeneratePipePath returns a platform-appropriate pipe path for the given name.
pub fn generate_pipe_path(name: &str) -> String {
    path::Path::new(&std::env::temp_dir())
        .join(name)
        .to_string_lossy()
        .into_owned()
}

struct UnixPipeListener {
    listener: UnixListener,
}

impl Listener for UnixPipeListener {
    fn accept(&mut self) -> io::Result<Box<dyn ReadWriteClose>> {
        let (stream, _) = self.listener.accept()?;
        Ok(Box::new(stream))
    }

    fn close(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn addr_string(&self) -> String {
        self.listener
            .local_addr()
            .ok()
            .and_then(|addr| addr.as_pathname().map(|path| path.display().to_string()))
            .unwrap_or_default()
    }
}

impl ReadWriteClose for UnixStream {
    fn close(&mut self) -> io::Result<()> {
        self.shutdown(std::net::Shutdown::Both)
    }

    fn clone_reader_writer(&self) -> Box<dyn ReadWriteClose> {
        Box::new(self.try_clone().expect("failed to clone UnixStream"))
    }
}
