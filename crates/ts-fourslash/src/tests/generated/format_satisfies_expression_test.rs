#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_satisfies_expression() {
    let mut t = TestingT;
    run_test_format_satisfies_expression(&mut t);
}

fn run_test_format_satisfies_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatSatisfiesExpression") {
        return;
    }
    let content = r#"type Foo = "a" | "b" | "c";
const foo1 = ["a"] satisfies Foo[];
const foo2 = ["a"]satisfies Foo[];
const foo3 = ["a"]  satisfies Foo[];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"type Foo = "a" | "b" | "c";
const foo1 = ["a"] satisfies Foo[];
const foo2 = ["a"] satisfies Foo[];
const foo3 = ["a"] satisfies Foo[];"#,
    );
    done();
}
