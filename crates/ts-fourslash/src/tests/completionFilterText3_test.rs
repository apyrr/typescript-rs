use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_filter_text3(t: &mut TestingT) {
    let content = r#"// @strict: true
declare const foo1: { b: number; "a bc": string; };
if (true) {
    foo1[|.|]/*1*/
} 
else {
    foo1[|.a|]/*2*/
}

declare const foo2: { b: number; "a bc": string; } | undefined;
if (true) {
    foo2[|.|]/*3*/
} else if (false) {
    foo2[|.a|]/*4*/
} else {
    foo2[|?.|]/*5*/
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    verify_a_bc_completion(
        &mut f,
        t,
        MarkerInput::Name("1".to_string()),
        ExpectedCompletionEditRange::None,
        a_bc_completion_item("[\"a bc\"]", ".a bc", ranges[0].ls_range),
    );
    verify_a_bc_completion(
        &mut f,
        t,
        MarkerInput::Name("2".to_string()),
        ExpectedCompletionEditRange::Ignored,
        a_bc_completion_item("[\"a bc\"]", ".a bc", ranges[1].ls_range),
    );
    verify_a_bc_completion(
        &mut f,
        t,
        MarkerInput::Name("3".to_string()),
        ExpectedCompletionEditRange::None,
        a_bc_completion_item("?.[\"a bc\"]", ".a bc", ranges[2].ls_range),
    );
    verify_a_bc_completion(
        &mut f,
        t,
        MarkerInput::Name("4".to_string()),
        ExpectedCompletionEditRange::Ignored,
        a_bc_completion_item("?.[\"a bc\"]", ".a bc", ranges[3].ls_range),
    );
    verify_a_bc_completion(
        &mut f,
        t,
        MarkerInput::Name("5".to_string()),
        ExpectedCompletionEditRange::None,
        a_bc_completion_item("?.[\"a bc\"]", "?.a bc", ranges[4].ls_range),
    );
    done();
}

fn verify_a_bc_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker_input: MarkerInput,
    edit_range: ExpectedCompletionEditRange,
    item: lsproto::CompletionItem,
) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range,
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

fn a_bc_completion_item(new_text: &str, filter_text: &str, range: lsproto::Range) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "a bc".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Field);
    item.sort_text = Some("11".to_string());
    item.insert_text = Some(new_text.to_string());
    item.filter_text = Some(filter_text.to_string());
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::TextEdit(
        lsproto::TextEdit {
            new_text: new_text.to_string(),
            range,
        },
    ));
    item
}

