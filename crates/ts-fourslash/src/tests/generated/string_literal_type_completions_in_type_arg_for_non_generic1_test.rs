#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_type_completions_in_type_arg_for_non_generic1() {
    let mut t = TestingT;
    run_test_string_literal_type_completions_in_type_arg_for_non_generic1(&mut t);
}

fn run_test_string_literal_type_completions_in_type_arg_for_non_generic1(t: &mut TestingT) {
    if should_skip_if_failing("TestStringLiteralTypeCompletionsInTypeArgForNonGeneric1") {
        return;
    }
    let content = r#"interface Foo {}
type Bar = {};

let x: Foo<"/*1*/">;
let y: Bar<"/*2*/">;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
