use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

// Where exhaustive case completions are available.
pub fn test_exhaustive_case_completions3(t: &mut TestingT) {
    let content = r#"// @newline: LF
// @Filename: /main.ts
enum E {
    A = 0,
    B = "B",
    C = "C",
}
declare const u: E;
switch (u) {
    case/*1*/
}
switch (u) {
    /*2*/
}
switch (u) {
    case 1:
    /*3*/
}
switch (u) {
    [|c|]/*4*/   
}
switch (u) {
    case /*5*/
}
/*6*/
switch (u) {
    /*7*/

switch (u) {
    case E./*8*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let replacement_range = f.ranges()[0].ls_range.clone();

    for marker in ["1", "2", "3", "5", "7"] {
        verify_includes(&mut f, t, marker, vec![case_completion_item("case E.A: ...")]);
    }
    verify_includes(
        &mut f,
        t,
        "4",
        vec![case_completion_item_with_text_edit(
            "case E.A: ...",
            replacement_range,
        )],
    );
    verify_includes(
        &mut f,
        t,
        "6",
        vec![
            CompletionsExpectedItem::Label("E".to_string()),
            CompletionsExpectedItem::Label("u".to_string()),
            case_completion_item("case E.A: ..."),
        ],
    );
    verify_exact(&mut f, t, "8", &["A", "B", "C"]);
    done();
}

fn verify_includes(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker: &str,
    includes: Vec<CompletionsExpectedItem>,
) {
    let expected = expected_list(includes, Vec::new());
    f.verify_completions(t, MarkerInput::Name(marker.to_string()), Some(&expected));
}

fn verify_exact(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str, labels: &[&str]) {
    let expected = expected_list(Vec::new(), completion_items(labels));
    f.verify_completions(t, MarkerInput::Name(marker.to_string()), Some(&expected));
}

fn expected_list(
    includes: Vec<CompletionsExpectedItem>,
    exact: Vec<CompletionsExpectedItem>,
) -> CompletionsExpectedList {
    CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes,
            excludes: Vec::new(),
            exact,
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    }
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn case_completion_item(label: &str) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.insert_text = Some("case E.A:$1\ncase E.B:$2\ncase E.C:$3".to_string());
    item.sort_text = Some("15".to_string());
    item.insert_text_format = Some(lsproto::InsertTextFormat::Snippet);
    CompletionsExpectedItem::Item(item)
}

fn case_completion_item_with_text_edit(label: &str, range: lsproto::Range) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.sort_text = Some("15".to_string());
    item.insert_text_format = Some(lsproto::InsertTextFormat::Snippet);
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::InsertReplaceEdit(
        lsproto::InsertReplaceEdit {
            new_text: "case E.A:$1\ncase E.B:$2\ncase E.C:$3".to_string(),
            insert: range.clone(),
            replace: range,
        },
    ));
    CompletionsExpectedItem::Item(item)
}

fn completion_items(values: &[&str]) -> Vec<CompletionsExpectedItem> {
    values
        .iter()
        .map(|value| CompletionsExpectedItem::Label((*value).to_string()))
        .collect()
}

