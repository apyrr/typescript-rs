#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_quoted_object_literal_union() {
    let mut t = TestingT;
    run_test_completions_quoted_object_literal_union(&mut t);
}

fn run_test_completions_quoted_object_literal_union(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface A {
  "a-prop": string;
}

interface B {
  "b-prop": string;
}

const obj: A | B = {
  "/*1*/"
}"#;
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
                exact: vec![
                    CompletionsExpectedItem::Label("a-prop".to_string()),
                    CompletionsExpectedItem::Label("b-prop".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
