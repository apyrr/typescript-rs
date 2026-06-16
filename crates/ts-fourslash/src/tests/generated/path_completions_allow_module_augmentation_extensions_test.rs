#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_path_completions_allow_module_augmentation_extensions() {
    let mut t = TestingT;
    run_test_path_completions_allow_module_augmentation_extensions(&mut t);
}

fn run_test_path_completions_allow_module_augmentation_extensions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /project/foo.css
export const foo = 0;
// @Filename: declarations.d.ts
declare module "*.css" {}
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
                exact: vec![CompletionsExpectedItem::Label("foo.css".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
