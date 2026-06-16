#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_import_clause01() {
    let mut t = TestingT;
    run_test_completion_list_in_import_clause01(&mut t);
}

fn run_test_completion_list_in_import_clause01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: m1.ts
export var foo: number = 1;
export function bar() { return 10; }
export function baz() { return 10; }
// @Filename: m2.ts
import {/*1*/, /*2*/ from "./m1"
import {/*3*/} from "./m1"
import {foo,/*4*/ from "./m1"
import {bar as /*5*/, /*6*/ from "./m1"
import {foo, bar, baz as b,/*7*/} from "./m1"
import { type /*8*/ } from "./m1";
import { type b/*9*/ } from "./m1";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["8".to_string(), "9".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("baz".to_string()),
                    CompletionsExpectedItem::Label("foo".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
