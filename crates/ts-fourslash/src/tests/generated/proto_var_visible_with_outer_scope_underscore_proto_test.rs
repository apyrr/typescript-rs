#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_proto_var_visible_with_outer_scope_underscore_proto() {
    let mut t = TestingT;
    run_test_proto_var_visible_with_outer_scope_underscore_proto(&mut t);
}

fn run_test_proto_var_visible_with_outer_scope_underscore_proto(t: &mut TestingT) {
    if should_skip_if_failing("TestProtoVarVisibleWithOuterScopeUnderscoreProto") {
        return;
    }
    let content = r#"// outer
var ___proto__ = 10;
function foo() {
    var __proto__ = "hello";
    /**/
}"#;
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
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "__proto__".to_string(),
                        detail: Some("(local var) __proto__: string".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "___proto__".to_string(),
                        detail: Some("var ___proto__: number".to_string()),
                        ..Default::default()
                    }),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
