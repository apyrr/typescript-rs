#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_export_equal_types() {
    let mut t = TestingT;
    run_test_export_equal_types(&mut t);
}

fn run_test_export_equal_types(t: &mut TestingT) {
    if should_skip_if_failing("TestExportEqualTypes") {
        return;
    }
    let content = r"// @module: commonjs
// @lib: es5
// @strict: false
// @Filename: exportEqualTypes_file0.ts
interface x {
    (): Date;
    foo: string;
}
export = x;
// @Filename: exportEqualTypes_file1.ts
///<reference path='exportEqualTypes_file0.ts'/>
import test = require('./exportEqualTypes_file0');
var t: /*1*/test;  // var 't' should be of type 'test'
var /*2*/r1 = t(); // Should return a Date
var /*3*/r2 = t./*4*/foo; // t should have 'foo' in dropdown list and be of type 'string'";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(alias) interface test\nimport test = require('./exportEqualTypes_file0')",
        "",
    );
    f.verify_quick_info_at(t, "2", "var r1: Date", "");
    f.verify_quick_info_at(t, "3", "var r2: string", "");
    f.verify_completions(
        t,
        MarkerInput::Name("4".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_function_members_with_prototype_plus(vec![
                    CompletionsExpectedItem::Label("foo".to_string()),
                ]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_no_errors();
    done();
}
