#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_invalid_member_names2() {
    let mut t = TestingT;
    run_test_completion_list_invalid_member_names2(&mut t);
}

fn run_test_completion_list_invalid_member_names2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
declare var Symbol: SymbolConstructor;
interface SymbolConstructor {
    readonly hasInstance: symbol;
}
interface Function {
    [Symbol.hasInstance](value: any): boolean;
}
interface SomeInterface {
    (value: number): any;
}
var _ : SomeInterface;
_./**/";
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
                exact: completion_function_members_with_prototype(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
