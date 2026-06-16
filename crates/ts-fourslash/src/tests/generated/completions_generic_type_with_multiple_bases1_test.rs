#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_generic_type_with_multiple_bases1() {
    let mut t = TestingT;
    run_test_completions_generic_type_with_multiple_bases1(&mut t);
}

fn run_test_completions_generic_type_with_multiple_bases1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsGenericTypeWithMultipleBases1") {
        return;
    }
    let content = r"export interface iBaseScope {
    watch: () => void;
}
export interface iMover {
    moveUp: () => void;
}
export interface iScope<TModel> extends iBaseScope, iMover {
    family: TModel;
}
var x: iScope<number>;
x./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
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
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "family".to_string(),
                        detail: Some("(property) iScope<number>.family: number".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "moveUp".to_string(),
                        detail: Some("(property) iMover.moveUp: () => void".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "watch".to_string(),
                        detail: Some("(property) iBaseScope.watch: () => void".to_string()),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
