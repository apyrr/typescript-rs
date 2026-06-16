use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completions_at_incomplete_object_literal_property(t: &mut TestingT) {
    let content = r#"// @noLib: true
f({
    [|a|]/**/
    xyz: ``,
});
declare function f(options: { abc?: number, xyz?: string }): void;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let range = f.ranges()[0].ls_range.clone();
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: vec![CompletionsExpectedItem::Item(optional_member_completion_item(range))],
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

fn optional_member_completion_item(range: lsproto::Range) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "abc?".to_string();
    item.filter_text = Some("abc".to_string());
    item.kind = Some(lsproto::CompletionItemKind::Field);
    item.sort_text = Some("12".to_string());
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::InsertReplaceEdit(
        lsproto::InsertReplaceEdit {
            new_text: "abc".to_string(),
            insert: range.clone(),
            replace: range,
        },
    ));
    item
}

