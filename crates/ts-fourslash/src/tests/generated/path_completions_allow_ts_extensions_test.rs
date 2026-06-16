#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_path_completions_allow_ts_extensions() {
    let mut t = TestingT;
    run_test_path_completions_allow_ts_extensions(&mut t);
}

fn run_test_path_completions_allow_ts_extensions(t: &mut TestingT) {
    if should_skip_if_failing("TestPathCompletionsAllowTsExtensions") {
        return;
    }
    let content = r#"// @moduleResolution: bundler
// @allowImportingTsExtensions: true
// @noEmit: true
// @Filename: /project/foo.ts
export const foo = 0;
// @Filename: /project/main.ts
import {} from ".//**/""#;
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
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("foo".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
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
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("foo.ts".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: Some(UserPreferences {
                import_module_specifier_ending:
                    modulespecifiers::ImportModuleSpecifierEndingPreference::Js,
                ..Default::default()
            }),
        }),
    );
    f.insert(t, "foo.ts\"\nimport {} from \"./");
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
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("foo.ts".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
