#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_nested_scopes() {
    let mut t = TestingT;
    run_test_formatting_nested_scopes(&mut t);
}

fn run_test_formatting_nested_scopes(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingNestedScopes") {
        return;
    }
    let content = r#"/*1*/        namespace      My.App      {
/*2*/export      var appModule =      angular.module("app", [
/*3*/            ]).config([() =>            {
/*4*/                        configureStates
/*5*/($stateProvider);
/*6*/}]).run(My.App.setup);
/*7*/      }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "namespace My.App {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    export var appModule = angular.module(\"app\", [");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    ]).config([() => {");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "        configureStates");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "            ($stateProvider);");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    }]).run(My.App.setup);");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "}");
    done();
}
