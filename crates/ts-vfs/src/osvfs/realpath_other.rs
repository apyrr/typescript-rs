use std::{fs, io, path::PathBuf};

pub fn realpath(path: &str) -> Result<String, io::Error> {
    let path: PathBuf = fs::canonicalize(path)?;
    Ok(path.to_string_lossy().into_owned())
}
