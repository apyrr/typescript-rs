#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_literal_from_inference_within_inferred_type1() {
    let mut t = TestingT;
    run_test_completions_literal_from_inference_within_inferred_type1(&mut t);
}

fn run_test_completions_literal_from_inference_within_inferred_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsLiteralFromInferenceWithinInferredType1") {
        return;
    }
    let content = r#"// @stableTypeOrdering: true
// @Filename: /a.tsx
declare function test<T>(a: {
  [K in keyof T]: {
    b?: keyof T;
  };
}): void;

test({
  foo: {},
  bar: {
    b: "/*ts*/",
  },
});

test({
  foo: {},
  bar: {
    b: /*ts2*/,
  },
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts".to_string()]),
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
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("foo".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts2".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("\"bar\"".to_string()),
                    CompletionsExpectedItem::Label("\"foo\"".to_string()),
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
