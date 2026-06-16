#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_shadowed_by_local() {
    let mut t = TestingT;
    run_test_completions_import_shadowed_by_local(&mut t);
}

fn run_test_completions_import_shadowed_by_local(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImport_shadowedByLocal") {
        return;
    }
    let content = r"// @noLib: true
// @Filename: /a.ts
export const foo = 0;
// @Filename: /b.ts
const foo = 1;
fo/**/";
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
                exact: completion_globals_plus(
                    vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "foo".to_string(),
                        detail: Some("const foo: 1".to_string()),
                        ..Default::default()
                    })],
                    true,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
