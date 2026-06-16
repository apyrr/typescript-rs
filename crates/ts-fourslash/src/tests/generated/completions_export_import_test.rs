#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_export_import() {
    let mut t = TestingT;
    run_test_completions_export_import(&mut t);
}

fn run_test_completions_export_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
declare global {
    namespace N {
        const foo: number;
    }
}
export import foo = N.foo;
/**/";
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
                    vec![
                        CompletionsExpectedItem::Item(lsproto::CompletionItem {
                            label: "foo".to_string(),
                            kind: Some(lsproto::CompletionItemKind::VARIABLE),
                            detail: Some(
                                "(alias) const foo: number\nimport foo = N.foo".to_string(),
                            ),
                            ..Default::default()
                        }),
                        CompletionsExpectedItem::Item(lsproto::CompletionItem {
                            label: "N".to_string(),
                            kind: Some(lsproto::CompletionItemKind::MODULE),
                            detail: Some("namespace N".to_string()),
                            ..Default::default()
                        }),
                    ],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
