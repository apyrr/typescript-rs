#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_object_binding_pattern12() {
    let mut t = TestingT;
    run_test_completion_list_in_object_binding_pattern12(&mut t);
}

fn run_test_completion_list_in_object_binding_pattern12(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInObjectBindingPattern12") {
        return;
    }
    let content = r"interface I {
    property1: number;
    property2: string;
}

function f({ property1, /**/ }: I): void {
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
                includes: vec![CompletionsExpectedItem::Label("property2".to_string())],
                excludes: vec!["property1".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
