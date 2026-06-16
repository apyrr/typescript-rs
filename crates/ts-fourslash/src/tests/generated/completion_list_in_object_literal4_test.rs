#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_object_literal4() {
    let mut t = TestingT;
    run_test_completion_list_in_object_literal4(&mut t);
}

fn run_test_completion_list_in_object_literal4(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInObjectLiteral4") {
        return;
    }
    let content = r"// @strictNullChecks: true
interface Thing {
    hello: number;
    world: string;
}

declare function funcA(x : Thing): void;
declare function funcB(x?: Thing): void;
declare function funcC(x : Thing | null): void;
declare function funcD(x : Thing | undefined): void;
declare function funcE(x : Thing | null | undefined): void;
declare function funcF(x?: Thing | null | undefined): void;

funcA({ /*A*/ });
funcB({ /*B*/ });
funcC({ /*C*/ });
funcD({ /*D*/ });
funcE({ /*E*/ });
funcF({ /*F*/ });";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
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
                    CompletionsExpectedItem::Label("hello".to_string()),
                    CompletionsExpectedItem::Label("world".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
