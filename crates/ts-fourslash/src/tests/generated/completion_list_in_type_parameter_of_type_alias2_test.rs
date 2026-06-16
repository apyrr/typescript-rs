#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_type_parameter_of_type_alias2() {
    let mut t = TestingT;
    run_test_completion_list_in_type_parameter_of_type_alias2(&mut t);
}

fn run_test_completion_list_in_type_parameter_of_type_alias2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInTypeParameterOfTypeAlias2") {
        return;
    }
    let content = r"type Map1<K, /*0*/
type Map1<K, /*1*/V> = [];
type Map1<K,V> = /*2*/[];
type Map1<K1, V1> = </*3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["0".to_string(), "1".to_string()]),
        None,
    );
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("K".to_string()),
                    CompletionsExpectedItem::Label("V".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(t, MarkerInput::Name("3".to_string()), None);
    done();
}
