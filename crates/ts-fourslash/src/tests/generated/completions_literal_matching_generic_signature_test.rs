#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_literal_matching_generic_signature() {
    let mut t = TestingT;
    run_test_completions_literal_matching_generic_signature(&mut t);
}

fn run_test_completions_literal_matching_generic_signature(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsLiteralMatchingGenericSignature") {
        return;
    }
    let content = r#"// @Filename: /a.tsx
declare function bar1<P extends "" | "bar" | "baz">(p: P): void;

bar1("/*ts*/")
"#;
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
                    CompletionsExpectedItem::Label("".to_string()),
                    CompletionsExpectedItem::Label("bar".to_string()),
                    CompletionsExpectedItem::Label("baz".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
