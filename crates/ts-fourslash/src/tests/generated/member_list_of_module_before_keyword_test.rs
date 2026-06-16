#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_of_module_before_keyword() {
    let mut t = TestingT;
    run_test_member_list_of_module_before_keyword(&mut t);
}

fn run_test_member_list_of_module_before_keyword(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListOfModuleBeforeKeyword") {
        return;
    }
    let content = r"namespace TypeModule1 {
    export class C1 { }
    export class C2 { }
}
var x: TypeModule1./*namedType*/
namespace TypeModule2 {
    export class Test3 {}
}

TypeModule1./*dottedExpression*/
namespace TypeModule3 {
    export class Test3 {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
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
                    CompletionsExpectedItem::Label("C1".to_string()),
                    CompletionsExpectedItem::Label("C2".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
