use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_exhaustive_case_completions1(t: &mut TestingT) {
    let content = r#"// @newline: LF
enum E {
    A = 0,
    B = "B",
    C = "C",
}
// Mixed union
declare const u: E.A | E.B | 1;
switch (u) {
    case/*1*/
}
// Union enum
declare const e: E;
switch (e) {
    case/*2*/
}
enum F {
    D = 1 << 0,
    E = 1 << 1,
    F = 1 << 2,
}

declare const f: F;
switch (f) {
    case/*3*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    verify_case_completion(&mut f, t, "1", "case 1: ...", "case 1:$1\ncase E.A:$2\ncase E.B:$3");
    verify_case_completion(&mut f, t, "2", "case E.A: ...", "case E.A:$1\ncase E.B:$2\ncase E.C:$3");
    verify_case_completion(&mut f, t, "3", "case F.D: ...", "case F.D:$1\ncase F.E:$2\ncase F.F:$3");
    done();
}

fn verify_case_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker: &str,
    label: &str,
    insert_text: &str,
) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![case_completion_item(label, insert_text)],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name(marker.to_string()), Some(&expected));
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

