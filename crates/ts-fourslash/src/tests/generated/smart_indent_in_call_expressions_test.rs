#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_in_call_expressions() {
    let mut t = TestingT;
    run_test_smart_indent_in_call_expressions(&mut t);
}

fn run_test_smart_indent_in_call_expressions(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentInCallExpressions") {
        return;
    }
    let content = r#"namespace My.App {
    export var appModule = angular.module("app", [
    ]).config([() => {
        configureStates/*1*/($stateProvider);
    }]).run(My.App.setup);
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 12);
    done();
}
