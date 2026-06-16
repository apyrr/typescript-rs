#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_function_declaration() {
    let mut t = TestingT;
    run_test_completion_list_in_function_declaration(&mut t);
}

fn run_test_completion_list_in_function_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
var a = 0;
function foo(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    f.insert(t, "a");
    f.verify_completions(t, MarkerInput::None, None);
    f.insert(t, " , ");
    f.verify_completions(t, MarkerInput::None, None);
    f.insert(t, "b");
    f.verify_completions(t, MarkerInput::None, None);
    f.insert(t, ":");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_global_types(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.insert(t, "number, ");
    f.verify_completions(t, MarkerInput::None, None);
    done();
}
