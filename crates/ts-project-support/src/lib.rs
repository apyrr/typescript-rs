#![forbid(unsafe_code)]

#[expect(dead_code, reason = "ported dirty-map API is ahead of current callers")]
pub mod dirty;
pub mod logging;
