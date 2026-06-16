use std::{fs, path::PathBuf};

use crate::vfs::Fs;

#[test]
fn test_os_read_file() {
    let fs = super::os::fs();
    let cargo_toml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let expected = fs::read_to_string(&cargo_toml).unwrap();
    let (contents, ok) = fs.read_file(&cargo_toml.to_string_lossy());
    assert!(ok);
    assert_eq!(contents, expected);
}

#[test]
fn test_os_realpath() {
    let fs = super::os::fs();
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    let realpath = fs.realpath(&home);
    assert!(!realpath.is_empty());
    #[cfg(windows)]
    {
        if home.len() >= 2 && home.as_bytes()[1] == b':' {
            assert_eq!(&realpath[..1], &home[..1].to_ascii_uppercase());
        }
    }
}

#[test]
fn test_os_use_case_sensitive_file_names() {
    let fs = super::os::fs();
    let value = fs.use_case_sensitive_file_names();
    #[cfg(windows)]
    assert!(!value);
    #[cfg(target_os = "linux")]
    assert!(value);
}
