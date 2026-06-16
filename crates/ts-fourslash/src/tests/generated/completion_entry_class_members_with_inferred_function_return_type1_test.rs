#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_entry_class_members_with_inferred_function_return_type1() {
    let mut t = TestingT;
    run_test_completion_entry_class_members_with_inferred_function_return_type1(&mut t);
}

fn run_test_completion_entry_class_members_with_inferred_function_return_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionEntryClassMembersWithInferredFunctionReturnType1") {
        return;
    }
    let content = r#"// @filename: /tokenizer.ts
export default abstract class Tokenizer {
  errorBuilder() {
    return (pos: number, lineStart: number, curLine: number) => {};
  }
}
// @filename: /expression.ts
import Tokenizer from "./tokenizer.js";

export default abstract class ExpressionParser extends Tokenizer {
  /**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&CompletionsExpectedList {
    is_incomplete: false,
    item_defaults: Some(CompletionsExpectedItemDefaults {
        commit_characters: Some(Vec::new()),
        edit_range: ExpectedCompletionEditRange::Ignored,
    }),
    items: Some(CompletionsExpectedItems {
        includes: vec![
CompletionsExpectedItem::Item(lsproto::CompletionItem {
        label: "errorBuilder".to_string(),
        insert_text: Some("errorBuilder(): (pos: number, lineStart: number, curLine: number) => void {\n})".to_string()),
        filter_text: Some("errorBuilder".to_string()),
        ..Default::default()
    }),
],
        excludes: Vec::new(),
        exact: Vec::new(),
        unsorted: Vec::new(),
    }),
    user_preferences: None,
}));
    done();
}
