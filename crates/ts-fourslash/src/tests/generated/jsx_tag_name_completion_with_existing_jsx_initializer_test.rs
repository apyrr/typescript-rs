#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_tag_name_completion_with_existing_jsx_initializer() {
    let mut t = TestingT;
    run_test_jsx_tag_name_completion_with_existing_jsx_initializer(&mut t);
}

fn run_test_jsx_tag_name_completion_with_existing_jsx_initializer(t: &mut TestingT) {
    if should_skip_if_failing("TestJsxTagNameCompletionWithExistingJsxInitializer") {
        return;
    }
    let content = r#"// @filename: /foo.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
        foo: {
            className: string;
        }
    }
}
<foo cl/**/={""} />"#;
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "className".to_string(),
                    detail: Some("(property) className: string".to_string()),
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
