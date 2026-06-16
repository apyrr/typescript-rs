use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT, UserPreferences,
};
use ts_lsproto as lsproto;

pub fn test_exhaustive_case_completions6(t: &mut TestingT) {
    let content = r#"// @newline: LF
declare const p: 'A' | 'B' | 'C';

switch (p) {
    /*1*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![case_completion_item(
                "case 'A': ...",
                "case 'A':$1\ncase 'B':$2\ncase 'C':$3",
            )],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: Some(UserPreferences {
            quote_preference: Some("single".to_string()),
            ..UserPreferences::default()
        }),
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

fn case_completion_item(label: &str, insert_text: &str) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.insert_text = Some(insert_text.to_string());
    item.sort_text = Some("15".to_string());
    item.insert_text_format = Some(lsproto::InsertTextFormat::Snippet);
    CompletionsExpectedItem::Item(item)
}

