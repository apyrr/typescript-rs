#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_of_enum_from_external_module() {
    let mut t = TestingT;
    run_test_member_list_of_enum_from_external_module(&mut t);
}

fn run_test_member_list_of_enum_from_external_module(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListOfEnumFromExternalModule") {
        return;
    }
    let content = r"// @Filename: memberListOfEnumFromExternalModule_file0.ts
export enum Topic{ One, Two }
var topic = Topic.One;
// @Filename: memberListOfEnumFromExternalModule_file1.ts
import t = require('./memberListOfEnumFromExternalModule_file0');
var topic = t.Topic./*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
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
                    CompletionsExpectedItem::Label("One".to_string()),
                    CompletionsExpectedItem::Label("Two".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
