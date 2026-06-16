use std::{
    panic::{UnwindSafe, catch_unwind},
    sync::OnceLock,
};

use crate::race;

pub fn assert_panics<F>(f: F, expected: &str)
where
    F: FnOnce() + UnwindSafe,
{
    let got = catch_unwind(f);
    assert!(got.is_err());
    let payload = got.err().unwrap();
    let got = if let Some(value) = payload.downcast_ref::<&str>() {
        (*value).to_owned()
    } else if let Some(value) = payload.downcast_ref::<String>() {
        value.clone()
    } else {
        String::new()
    };
    assert_eq!(got, expected);
}

pub fn recover_and_fail(msg: &str, f: impl FnOnce() + UnwindSafe) {
    if let Err(err) = catch_unwind(f) {
        panic!("{msg}:\n{err:?}");
    }
}

static TEST_PROGRAM_IS_SINGLE_THREADED: OnceLock<bool> = OnceLock::new();

pub fn test_program_is_single_threaded() -> bool {
    *TEST_PROGRAM_IS_SINGLE_THREADED.get_or_init(|| {
        // Leave Program in SingleThreaded mode unless explicitly configured or in race mode.
        if let Ok(value) = std::env::var("TS_TEST_PROGRAM_SINGLE_THREADED")
            && let Ok(value) = value.parse::<bool>()
        {
            return value;
        }
        !race::ENABLED
    })
}
