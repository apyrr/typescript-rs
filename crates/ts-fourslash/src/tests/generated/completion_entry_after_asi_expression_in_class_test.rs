#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_entry_after_asi_expression_in_class() {
    let mut t = TestingT;
    run_test_completion_entry_after_asi_expression_in_class(&mut t);
}

fn run_test_completion_entry_after_asi_expression_in_class(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionEntryAfterASIExpressionInClass") {
        return;
    }
    let content = r"class Parent {
  protected shouldWork() {
      console.log();
  }
}

class Child extends Parent {
            // this assumes ASI, but on next line wants to  
  x = () => 1
  shoul/*insideid*/ 
}

class ChildTwo extends Parent {
            // this assumes ASI, but on next line wants to  
  x = () => 1
  /*root*/ //nothing
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["insideid".to_string(), "root".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("shouldWork".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
