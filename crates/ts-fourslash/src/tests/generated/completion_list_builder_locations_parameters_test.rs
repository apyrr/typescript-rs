#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_builder_locations_parameters() {
    let mut t = TestingT;
    run_test_completion_list_builder_locations_parameters(&mut t);
}

fn run_test_completion_list_builder_locations_parameters(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListBuilderLocations_parameters") {
        return;
    }
    let content = r"var aa = 1;
class bar1{ constructor(/*1*/
class bar2{ constructor(a/*2*/
class bar3{ constructor(a, /*3*/
class bar4{ constructor(a, b/*4*/
class bar6{ constructor(public a, /*5*/
class bar7{ constructor(private a, /*6*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_constructor_parameter_keywords(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
