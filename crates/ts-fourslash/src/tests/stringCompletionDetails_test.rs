use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_string_completion_details(t: &mut TestingT) {
    let content = r#"const a: "aa" | "bb" = "/**/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::None,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(aa_completion_item())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}

fn aa_completion_item() -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "aa".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Constant);
    item.detail = Some("aa".to_string());
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::TextEdit(lsproto::TextEdit {
        range: lsproto::Range {
            start: lsproto::Position {
                line: 0,
                character: 24,
            },
            end: lsproto::Position {
                line: 0,
                character: 24,
            },
        },
        new_text: "aa".to_string(),
    }));
    item
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}
