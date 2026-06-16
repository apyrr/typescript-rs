use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_filter_text4(t: &mut TestingT) {
    let content = r#"declare const x: [number, number];
x[|.|]/**/;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::None,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(tuple_index_completion_item(
                ranges[0].ls_range,
            ))],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&expected));
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn tuple_index_completion_item(range: lsproto::Range) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "0".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Field);
    item.sort_text = Some("11".to_string());
    item.insert_text = Some("[0]".to_string());
    item.filter_text = Some(".0".to_string());
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::TextEdit(
        lsproto::TextEdit {
            new_text: "[0]".to_string(),
            range,
        },
    ));
    item
}

