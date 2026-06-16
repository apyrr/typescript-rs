#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_type_only_namespace() {
    let mut t = TestingT;
    run_test_completions_type_only_namespace(&mut t);
}

fn run_test_completions_type_only_namespace(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsTypeOnlyNamespace") {
        return;
    }
    let content = r"// @Filename: /a.ts
export namespace ns {
  export class Box<T> {}
  export type Type = {};
  export const Value = {};
}
// @Filename: /b.ts
import type { ns } from './a';
let x: ns./**/";
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
                        label: "Box".to_string(),
                        detail: Some("class ns.Box<T>".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "Type".to_string(),
                        detail: Some("type ns.Type = {})".to_string()),
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
