// Only Windows has reparse points; leave this nil for other OSes.
#![allow(dead_code)]

pub fn is_reparse_point(_path: &str) -> Option<bool> {
    None
}
