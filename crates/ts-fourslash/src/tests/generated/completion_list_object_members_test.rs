#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_object_members() {
    let mut t = TestingT;
    run_test_completion_list_object_members(&mut t);
}

fn run_test_completion_list_object_members(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListObjectMembers") {
        return;
    }
    let content = r" var object: {
     (bar: any): any;
     new (bar: any): any;
     [bar: any]: any;
     bar: any;
     foo(bar: any): any;
 };
object./**/";
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
                        label: "bar".to_string(),
                        detail: Some("(property) bar: any".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "foo".to_string(),
                        detail: Some("(method) foo(bar: any): any".to_string()),
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
