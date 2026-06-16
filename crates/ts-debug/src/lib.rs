#![forbid(unsafe_code)]
use std::fmt;

#[cfg(test)]
mod debug_test;

pub trait KindString {
    fn kind_string(&self) -> String;
}

pub fn fail(reason: &str) -> ! {
    let reason = if reason.is_empty() {
        "Debug failure.".to_string()
    } else {
        format!("Debug failure. {reason}")
    };
    // runtime.Breakpoint()
    panic!("{reason}");
}

pub fn fail_bad_syntax_kind(node: &impl KindString, message: Option<String>) -> ! {
    let msg = message.unwrap_or_else(|| "Unexpected node.".to_string());
    fail(&format!(
        "{}\nNode {} was unexpected.",
        msg,
        node.kind_string()
    ))
}

pub fn assert_never(member: &impl fmt::Display, message: Option<String>) -> ! {
    let msg = message.unwrap_or_else(|| "Illegal value:".to_string());
    fail(&format!("{msg} {member}"))
}

pub fn assert(value: bool, message: Option<String>) {
    if value {
        return;
    }
    assert_slow(message);
}

fn assert_slow(message: Option<String>) -> ! {
    // See https://dave.cheney.net/2020/05/02/mid-stack-inlining-in-go
    let msg = if let Some(message) = message {
        format!("False expression: {message}")
    } else {
        "False expression.".to_string()
    };
    fail(&msg)
}
