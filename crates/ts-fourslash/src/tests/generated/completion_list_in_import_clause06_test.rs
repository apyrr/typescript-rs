#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_import_clause06() {
    let mut t = TestingT;
    run_test_completion_list_in_import_clause06(&mut t);
}

fn run_test_completion_list_in_import_clause06(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @typeRoots: T1,T2
// @Filename: app.ts
import * as A from "/*1*/";
// @Filename: T1/a__b/index.d.ts
export declare let x: number;
// @Filename: T2/a__b/index.d.ts
export declare let x: number;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("@a/b".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
