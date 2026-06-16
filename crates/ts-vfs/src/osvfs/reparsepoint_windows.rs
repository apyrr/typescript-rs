use std::{fs, os::windows::fs::MetadataExt};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

pub fn is_reparse_point(path: &str) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .unwrap_or_default()
}
