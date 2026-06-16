#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_for_transitively_exported_members01() {
    let mut t = TestingT;
    run_test_completion_list_for_transitively_exported_members01(&mut t);
}

fn run_test_completion_list_for_transitively_exported_members01(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListForTransitivelyExportedMembers01") {
        return;
    }
    let content = r#"// @Filename: A.ts
export interface I1 { one: number }
export interface I2 { two: string }
export type I1_OR_I2 = I1 | I2;

export class C1 {
    one: string;
}

export namespace Inner {
    export interface I3 {
        three: boolean
    }

    export var varVar = 100;
    export let letVar = 200;
    export const constVar = 300;
}
// @Filename: B.ts
export var bVar = "bee!";
// @Filename: C.ts
export var cVar = "see!";
export * from "./A";
export * from "./B"
// @Filename: D.ts
import * as c from "./C";
var x = c./**/"#;
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
                    CompletionsExpectedItem::Label("bVar".to_string()),
                    CompletionsExpectedItem::Label("C1".to_string()),
                    CompletionsExpectedItem::Label("cVar".to_string()),
                    CompletionsExpectedItem::Label("Inner".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
