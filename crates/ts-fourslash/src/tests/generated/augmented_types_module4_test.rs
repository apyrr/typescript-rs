#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_augmented_types_module4() {
    let mut t = TestingT;
    run_test_augmented_types_module4(&mut t);
}

fn run_test_augmented_types_module4(t: &mut TestingT) {
    if should_skip_if_failing("TestAugmentedTypesModule4") {
        return;
    }
    let content = r"namespace m3d { export var y = 2; }
declare class m3d { foo(): void }
var /*1*/r = new m3d();
r./*2*/
var /*4*/r2 = m3d./*3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var r: m3d", "");
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
                exact: vec![CompletionsExpectedItem::Label("foo".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.insert(t, "foo();");
    f.verify_completions(
        t,
        MarkerInput::Name("3".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("y".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.insert(t, "y;");
    f.verify_quick_info_at(t, "4", "var r2: number", "");
    done();
}
