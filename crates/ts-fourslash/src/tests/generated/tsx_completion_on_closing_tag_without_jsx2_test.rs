#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_completion_on_closing_tag_without_jsx2() {
    let mut t = TestingT;
    run_test_tsx_completion_on_closing_tag_without_jsx2(&mut t);
}

fn run_test_tsx_completion_on_closing_tag_without_jsx2(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxCompletionOnClosingTagWithoutJSX2") {
        return;
    }
    let content = r"//@Filename: file.tsx
var x1 = <div>
   <h1> Hello world </ /*2*/>
   </ /*1*/>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("div".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("h1".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
