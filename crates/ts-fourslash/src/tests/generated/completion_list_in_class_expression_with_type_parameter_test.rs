#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_class_expression_with_type_parameter() {
    let mut t = TestingT;
    run_test_completion_list_in_class_expression_with_type_parameter(&mut t);
}

fn run_test_completion_list_in_class_expression_with_type_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInClassExpressionWithTypeParameter") {
        return;
    }
    let content = r"var x = class myClass <TypeParam> {
   getClassName (){
       /*0*/
       var tmp: /*0Type*/;
   }
   prop: Ty/*1*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("0".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["TypeParam".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["0Type".to_string(), "1".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "TypeParam".to_string(),
                    detail: Some("(type parameter) TypeParam in myClass<TypeParam>".to_string()),
                    kind: Some(lsproto::CompletionItemKind::PROPERTY),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
