use std::{
    cell::OnceCell,
    fs, io,
    path::{Path, PathBuf},
};

pub trait Fixture {
    fn name(&self) -> &str;
    fn path(&self) -> &Path;
    fn skip_if_not_exist(&self);
    fn read_file(&self) -> String;
}

struct FromFile {
    name: String,
    path: PathBuf,
    contents: OnceCell<Result<String, io::Error>>,
}

pub fn from_file(name: impl Into<String>, path: impl Into<PathBuf>) -> Box<dyn Fixture> {
    Box::new(FromFile {
        name: name.into(),
        path: path.into(),
        // Cache the file contents and errors.
        contents: OnceCell::new(),
    })
}

impl Fixture for FromFile {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn skip_if_not_exist(&self) {
        if fs::metadata(&self.path).is_err() {
            panic!("Test fixture {:?} does not exist", self.path);
        }
    }

    fn read_file(&self) -> String {
        match self.contents.get_or_init(|| fs::read_to_string(&self.path)) {
            Ok(contents) => contents.clone(),
            Err(err) => panic!("Failed to read test fixture {:?}: {err}", self.path),
        }
    }
}

struct FromString {
    name: String,
    path: PathBuf,
    contents: String,
}

pub fn from_string(
    name: impl Into<String>,
    path: impl Into<PathBuf>,
    contents: impl Into<String>,
) -> Box<dyn Fixture> {
    Box::new(FromString {
        name: name.into(),
        path: path.into(),
        contents: contents.into(),
    })
}

impl Fixture for FromString {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn skip_if_not_exist(&self) {}

    fn read_file(&self) -> String {
        self.contents.clone()
    }
}
