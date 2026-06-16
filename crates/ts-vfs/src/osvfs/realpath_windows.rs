use std::{fs, io};

pub fn realpath(path: &str) -> Result<String, io::Error> {
    let path = fs::canonicalize(path)?;
    let mut path = path.to_string_lossy().into_owned();
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        return Ok(format!(r"\\{stripped}"));
    }
    if let Some(stripped) = path.strip_prefix(r"\\?\") {
        path = stripped.to_string();
    }
    Ok(path)
}
