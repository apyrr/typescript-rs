use std::{fs, io};

pub fn realpath(path: &str) -> Result<String, io::Error> {
    fs::canonicalize(path).map(|path| path.to_string_lossy().into_owned())
}
