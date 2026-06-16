use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_filter_text2(t: &mut TestingT) {
    let content = r#"// @strict: true
declare const foo1: { bar: string } | undefined;
if (true) {
    foo1[|.|]/*1*/
}
else {
    foo1?./*2*/
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("1".to_string()),
        optional_chain_bar_completion_item(ranges[0].ls_range),
    );
    verify_bar_completion(
        &mut f,
        t,
        MarkerInput::Name("2".to_string()),
        bar_completion_item(),
    );
    done();
}

fn verify_bar_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker_input: MarkerInput,
    item: lsproto::CompletionItem,
) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::None,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(item)],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, marker_input, Some(&expected));
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn bar_completion_item() -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "bar".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Field);
    item.sort_text = Some("11".to_string());
    item
}

fn optional_chain_bar_completion_item(range: lsproto::Range) -> lsproto::CompletionItem {
    let mut item = bar_completion_item();
    item.insert_text = Some("?.bar".to_string());
    item.filter_text = Some(".bar".to_string());
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::TextEdit(
        lsproto::TextEdit {
            new_text: "?.bar".to_string(),
            range,
        },
    ));
    item
}

