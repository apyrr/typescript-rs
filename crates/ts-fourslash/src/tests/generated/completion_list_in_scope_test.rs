#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_scope() {
    let mut t = TestingT;
    run_test_completion_list_in_scope(&mut t);
}

fn run_test_completion_list_in_scope(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace TestModule {
    var localVariable = "";
    export var exportedVariable = 0;

    function localFunction() { }
    export function exportedFunction() { }

    class localClass { }
    export class exportedClass { }

    interface localInterface {}
    export interface exportedInterface {}

    namespace localModule {
        export var x = 0;
    }
    export namespace exportedModule {
        export var x = 0;
    }

    var v = /*valueReference*/ 0;
    var t :/*typeReference*/;
}

// Add some new items to the module
namespace TestModule {
    var localVariable2 = "";
    export var exportedVariable2 = 0;

    function localFunction2() { }
    export function exportedFunction2() { }

    class localClass2 { }
    export class exportedClass2 { }

    interface localInterface2 {}
    export interface exportedInterface2 {}

    namespace localModule2 {
        export var x = 0;
    }
    export namespace exportedModule2 {
        export var x = 0;
    }
}
var globalVar: string = "";
function globalFunction() { }

class TestClass {
    property: number;
    method() { }
    staticMethod() { }
    testMethod(param: number) {
        var localVar = 0;
        function localFunction() {};
        /*insideMethod*/
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("valueReference".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("localVariable".to_string()),
                    CompletionsExpectedItem::Label("exportedVariable".to_string()),
                    CompletionsExpectedItem::Label("localFunction".to_string()),
                    CompletionsExpectedItem::Label("exportedFunction".to_string()),
                    CompletionsExpectedItem::Label("localClass".to_string()),
                    CompletionsExpectedItem::Label("exportedClass".to_string()),
                    CompletionsExpectedItem::Label("localModule".to_string()),
                    CompletionsExpectedItem::Label("exportedModule".to_string()),
                    CompletionsExpectedItem::Label("exportedVariable2".to_string()),
                    CompletionsExpectedItem::Label("exportedFunction2".to_string()),
                    CompletionsExpectedItem::Label("exportedClass2".to_string()),
                    CompletionsExpectedItem::Label("exportedModule2".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("typeReference".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("localInterface".to_string()),
                    CompletionsExpectedItem::Label("exportedInterface".to_string()),
                    CompletionsExpectedItem::Label("localClass".to_string()),
                    CompletionsExpectedItem::Label("exportedClass".to_string()),
                    CompletionsExpectedItem::Label("exportedClass2".to_string()),
                ],
                excludes: vec![
                    "localModule".to_string(),
                    "exportedModule".to_string(),
                    "exportedModule2".to_string(),
                ],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("insideMethod".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("globalVar".to_string()),
                    CompletionsExpectedItem::Label("globalFunction".to_string()),
                    CompletionsExpectedItem::Label("param".to_string()),
                    CompletionsExpectedItem::Label("localVar".to_string()),
                    CompletionsExpectedItem::Label("localFunction".to_string()),
                ],
                excludes: vec![
                    "property".to_string(),
                    "testMethod".to_string(),
                    "staticMethod".to_string(),
                ],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
