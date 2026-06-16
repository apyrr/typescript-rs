#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_generic_indexed_access1() {
    let mut t = TestingT;
    run_test_completions_generic_indexed_access1(&mut t);
}

fn run_test_completions_generic_indexed_access1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsGenericIndexedAccess1") {
        return;
    }
    let content = r"interface Sample {
  addBook: { name: string, year: number }
}

export declare function testIt<T>(method: T[keyof T]): any
testIt<Sample>({ /**/ });";
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
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "name".to_string(),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "year".to_string(),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
