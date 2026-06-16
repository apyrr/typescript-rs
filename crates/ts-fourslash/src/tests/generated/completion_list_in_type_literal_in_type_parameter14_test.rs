#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_type_literal_in_type_parameter14() {
    let mut t = TestingT;
    run_test_completion_list_in_type_literal_in_type_parameter14(&mut t);
}

fn run_test_completion_list_in_type_literal_in_type_parameter14(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Foo {
   one: string;
   two: number;
}
declare function f<T extends Foo>(x: TemplateStringsArray): void;
f<{/*0*/}>` + "`" + `` + "`" + `;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("0".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("one".to_string()),
                    CompletionsExpectedItem::Label("two".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
