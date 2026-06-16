use crate::{
    get_default_capabilities, new_fourslash, CompletionsExpectedItem,
    CompletionsExpectedItemDefaults, CompletionsExpectedItems, CompletionsExpectedList,
    ExpectedCompletionEditRange, MarkerInput, TestingT,
};
use ts_lsproto as lsproto;

pub fn test_exhaustive_case_completions10(t: &mut TestingT) {
    let content = r#"
declare const u: "$1" | "2";
switch (u) {
    case/*1*/
}"#;
    let mut capabilities = get_default_capabilities();
    capabilities
        .text_document
        .completion
        .completion_item
        .snippet_support = false;
    let (mut f, done) = new_fourslash(t, Some(capabilities), content.to_string());
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(case_completion_item())],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("1".to_string()), Some(&expected));
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn case_completion_item() -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "case \"$1\": ...".to_string();
    item.insert_text = Some("case \"$1\":\ncase \"2\":".to_string());
    item.sort_text = Some("15".to_string());
    item
}

