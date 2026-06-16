#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_completions_for_open_ended_template_literal_type() {
    let mut t = TestingT;
    run_test_string_literal_completions_for_open_ended_template_literal_type(&mut t);
}

fn run_test_string_literal_completions_for_open_ended_template_literal_type(t: &mut TestingT) {
    if should_skip_if_failing("TestStringLiteralCompletionsForOpenEndedTemplateLiteralType") {
        return;
    }
    let content = r#"// @stableTypeOrdering: true
function conversionTest(groupName: | "downcast" | "dataDowncast" | "editingDowncast" | `${string}Downcast` & {}) {}
conversionTest("/**/");"#;
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("dataDowncast".to_string()),
                    CompletionsExpectedItem::Label("downcast".to_string()),
                    CompletionsExpectedItem::Label("editingDowncast".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
