#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_import_module_specifier_ending_unsupported_extension() {
    let mut t = TestingT;
    run_test_completion_import_module_specifier_ending_unsupported_extension(&mut t);
}

fn run_test_completion_import_module_specifier_ending_unsupported_extension(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename:index.css
 body {}
//@Filename:module.ts
import ".//**/""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["index.css".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: Some(UserPreferences {
                import_module_specifier_ending:
                    modulespecifiers::ImportModuleSpecifierEndingPreference::Js,
                ..Default::default()
            }),
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["index".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: Some(UserPreferences {
                import_module_specifier_ending:
                    modulespecifiers::ImportModuleSpecifierEndingPreference::Index,
                ..Default::default()
            }),
        }),
    );
    done();
}
