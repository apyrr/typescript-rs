#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal11() {
    let mut t = TestingT;
    run_test_completion_for_string_literal11(&mut t);
}

fn run_test_completion_for_string_literal11(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionForStringLiteral11") {
        return;
    }
    let content = r"// @stableTypeOrdering: true
type As = 'arf' | 'abacus' | 'abaddon';
let a: As;
switch (a) {
    case '[|/**/|]
}";
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
                    CompletionsExpectedItem::Label("abacus".to_string()),
                    CompletionsExpectedItem::Label("abaddon".to_string()),
                    CompletionsExpectedItem::Label("arf".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
