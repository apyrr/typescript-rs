#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_builder_locations_variable_declarations() {
    let mut t = TestingT;
    run_test_completion_list_builder_locations_variable_declarations(&mut t);
}

fn run_test_completion_list_builder_locations_variable_declarations(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListBuilderLocations_VariableDeclarations") {
        return;
    }
    let content = r#"// @lib: es5
var x = a/*var1*/
var x = (b/*var2*/
var x = (c, d/*var3*/
 var y : any = "", x = a/*var4*/
 var y : any = "", x = (a/*var5*/
class C{}
var y = new C(/*var6*/
 class C{}
 var y = new C(0, /*var7*/
var y = [/*var8*/
var y = [0, /*var9*/
var y = `${/*var10*/
var y = `${10} dd ${ /*var11*/
var y = 10; y=/*var12*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["var1".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals_plus(
                    vec![
                        CompletionsExpectedItem::Label("C".to_string()),
                        CompletionsExpectedItem::Label("y".to_string()),
                    ],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "var2".to_string(),
            "var3".to_string(),
            "var4".to_string(),
            "var5".to_string(),
            "var6".to_string(),
            "var7".to_string(),
            "var8".to_string(),
            "var9".to_string(),
            "var10".to_string(),
            "var11".to_string(),
            "var12".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals_plus(
                    vec![
                        CompletionsExpectedItem::Label("C".to_string()),
                        CompletionsExpectedItem::Label("x".to_string()),
                        CompletionsExpectedItem::Label("y".to_string()),
                    ],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
