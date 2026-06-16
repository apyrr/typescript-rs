#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_identifier_definition_locations_properties() {
    let mut t = TestingT;
    run_test_completion_list_at_identifier_definition_locations_properties(&mut t);
}

fn run_test_completion_list_at_identifier_definition_locations_properties(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var aa = 1;
class A1 {
    /*property1*/
}
class A2 {
    p/*property2*/
}
class A3 {
    public s/*property3*/
}
class A4 {
    a/*property4*/
}
class A5 {
    public a/*property5*/
}
class A6 {
    protected a/*property6*/
}
class A7 {
    private a/*property7*/
}";
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
                exact: completion_class_element_keywords(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
