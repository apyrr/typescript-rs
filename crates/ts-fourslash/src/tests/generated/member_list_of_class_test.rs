#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_of_class() {
    let mut t = TestingT;
    run_test_member_list_of_class(&mut t);
}

fn run_test_member_list_of_class(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListOfClass") {
        return;
    }
    let content = r"class C1 {
   public pubMeth() { }
   private privMeth() { }
   public pubProp = 0;
   private privProp = 0;
}
var f = new C1();
f./**/";
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "pubMeth".to_string(),
                        detail: Some("(method) C1.pubMeth(): void".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "pubProp".to_string(),
                        detail: Some("(property) C1.pubProp: number".to_string()),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
