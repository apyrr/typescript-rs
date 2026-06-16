use std::fmt;

use crate::*;

fn assert_panics(f: impl FnOnce() + std::panic::UnwindSafe, expected: &str) {
    let err = std::panic::catch_unwind(f).expect_err("expected panic");
    let actual = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert_eq!(actual, expected);
}

#[test]
fn fail_empty_reason() {
    assert_panics(|| fail(""), "Debug failure.");
}

#[test]
fn fail_with_reason() {
    assert_panics(
        || fail("something went wrong"),
        "Debug failure. something went wrong",
    );
}

struct MockNode {
    kind: String,
}

impl KindString for MockNode {
    fn kind_string(&self) -> String {
        self.kind.clone()
    }
}

impl fmt::Display for MockNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kind)
    }
}

#[test]
fn fail_bad_syntax_kind_no_message() {
    assert_panics(
        || {
            fail_bad_syntax_kind(
                &MockNode {
                    kind: "FooNode".into(),
                },
                None,
            )
        },
        "Debug failure. Unexpected node.\nNode FooNode was unexpected.",
    );
}

#[test]
fn fail_bad_syntax_kind_with_message() {
    assert_panics(
        || {
            fail_bad_syntax_kind(
                &MockNode {
                    kind: "BarNode".into(),
                },
                Some("custom message".into()),
            )
        },
        "Debug failure. custom message\nNode BarNode was unexpected.",
    );
}

#[test]
fn assert_never_default_message_kind_string() {
    assert_panics(
        || {
            assert_never(
                &MockNode {
                    kind: "TestNode".into(),
                },
                None,
            )
        },
        "Debug failure. Illegal value: TestNode",
    );
}

#[test]
fn assert_never_custom_message_kind_string() {
    assert_panics(
        || {
            assert_never(
                &MockNode {
                    kind: "TestNode".into(),
                },
                Some("bad value:".into()),
            )
        },
        "Debug failure. bad value: TestNode",
    );
}

struct MockStringer {
    s: String,
}

impl fmt::Display for MockStringer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.s)
    }
}

#[test]
fn assert_never_stringer() {
    assert_panics(
        || assert_never(&MockStringer { s: "hello".into() }, None),
        "Debug failure. Illegal value: hello",
    );
}

#[test]
fn assert_never_fallback() {
    assert_panics(
        || assert_never(&42, None),
        "Debug failure. Illegal value: 42",
    );
}

#[test]
fn assert_true() {
    assert(true, None);
}

#[test]
fn assert_true_with_message() {
    assert(true, Some("this should not trigger".into()));
}

#[test]
fn assert_false_no_message() {
    assert_panics(|| assert(false, None), "Debug failure. False expression.");
}

#[test]
fn assert_false_with_message() {
    assert_panics(
        || assert(false, Some("expected x > 0".into())),
        "Debug failure. False expression: expected x > 0",
    );
}
