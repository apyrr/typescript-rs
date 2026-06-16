#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_after_class_extends() {
    let mut t = TestingT;
    run_test_completion_list_after_class_extends(&mut t);
}

fn run_test_completion_list_after_class_extends(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAfterClassExtends") {
        return;
    }
    let content = r"namespace Bar {
    export class Bleah {
    }
    export class Foo extends /**/Bleah {
    }
}

function Blah(x: Bar.Bleah) {
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
                includes: vec![
                    CompletionsExpectedItem::Label("Bar".to_string()),
                    CompletionsExpectedItem::Label("Bleah".to_string()),
                    CompletionsExpectedItem::Label("Foo".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
