#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_module_members() {
    let mut t = TestingT;
    run_test_completion_list_module_members(&mut t);
}

fn run_test_completion_list_module_members(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListModuleMembers") {
        return;
    }
    let content = r" namespace Module {
     var innerVariable = 1;
     function innerFunction() { }
     class innerClass { }
     namespace innerModule { }
     interface innerInterface {}
     export var exportedVariable = 1;
     export function exportedFunction() { }
     export class exportedClass { }
     export namespace exportedModule { export var exportedInnerModuleVariable = 1; }
     export interface exportedInterface {}
 }

Module./*ValueReference*/;

var x : Module./*TypeReference*/

class TestClass extends Module./*TypeReferenceInExtendsList*/ { }

interface TestInterface implements Module./*TypeReferenceInImplementsList*/ { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "ValueReference".to_string(),
            "TypeReferenceInExtendsList".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("exportedFunction".to_string()),
                    CompletionsExpectedItem::Label("exportedVariable".to_string()),
                    CompletionsExpectedItem::Label("exportedClass".to_string()),
                    CompletionsExpectedItem::Label("exportedModule".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "TypeReference".to_string(),
            "TypeReferenceInImplementsList".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("exportedClass".to_string()),
                    CompletionsExpectedItem::Label("exportedInterface".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
