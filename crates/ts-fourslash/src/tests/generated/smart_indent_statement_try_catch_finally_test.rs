#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_statement_try_catch_finally() {
    let mut t = TestingT;
    run_test_smart_indent_statement_try_catch_finally(&mut t);
}

fn run_test_smart_indent_statement_try_catch_finally(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentStatementTryCatchFinally") {
        return;
    }
    let content = r#"function tryCatch() {
    {| "indentation": 4 |}
    try {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
    catch (err) {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
}

function tryFinally() {
    {| "indentation": 4 |}
    try {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
    finally {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
}

function tryCatchFinally() {
    {| "indentation": 4 |}
    try {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
    catch (err) {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
    finally {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
