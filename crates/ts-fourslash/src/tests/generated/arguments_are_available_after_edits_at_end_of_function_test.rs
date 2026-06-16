#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_arguments_are_available_after_edits_at_end_of_function() {
    let mut t = TestingT;
    run_test_arguments_are_available_after_edits_at_end_of_function(&mut t);
}

fn run_test_arguments_are_available_after_edits_at_end_of_function(t: &mut TestingT) {
    if should_skip_if_failing("TestArgumentsAreAvailableAfterEditsAtEndOfFunction") {
        return;
    }
    let content = r"namespace Test1 {
	class Person {
		children: string[];
		constructor(public name: string, children: string[]) {
			/**/
		}
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "this.children = ch");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "children".to_string(),
                    detail: Some("(parameter) children: string[]".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
