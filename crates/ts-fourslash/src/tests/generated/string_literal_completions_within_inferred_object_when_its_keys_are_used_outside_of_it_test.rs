#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_completions_within_inferred_object_when_its_keys_are_used_outside_of_it()
{
    let mut t = TestingT;
    run_test_string_literal_completions_within_inferred_object_when_its_keys_are_used_outside_of_it(
        &mut t,
    );
}

fn run_test_string_literal_completions_within_inferred_object_when_its_keys_are_used_outside_of_it(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"// @strict: true
declare function createMachine<T>(config: {
  initial: keyof T;
  states: {
    [K in keyof T]: {
      on?: Record<string, keyof T>;
    };
  };
}): void;

createMachine({
  initial: "a",
  states: {
    a: {
      on: {
        NEXT: "/*1*/",
      },
    },
    b: {
      on: {
        NEXT: "/*2*/",
      },
    },
  },
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "2".to_string()]),
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
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
