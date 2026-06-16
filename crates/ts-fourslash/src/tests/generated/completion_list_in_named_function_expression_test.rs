#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_named_function_expression() {
    let mut t = TestingT;
    run_test_completion_list_in_named_function_expression(&mut t);
}

fn run_test_completion_list_in_named_function_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function foo(a: number): string {
    /*insideFunctionDeclaration*/
    return "";
}

(function foo(): number {
    /*insideFunctionExpression*/
    fo/*referenceInsideFunctionExpression*/o;
    return "";
})

/*globalScope*/
fo/*referenceInGlobalScope*/o;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "globalScope".to_string(),
            "insideFunctionDeclaration".to_string(),
            "insideFunctionExpression".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("foo".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_quick_info_at(
        t,
        "referenceInsideFunctionExpression",
        "(local function) foo(): number",
        "",
    );
    f.verify_quick_info_at(
        t,
        "referenceInGlobalScope",
        "function foo(a: number): string",
        "",
    );
    done();
}
