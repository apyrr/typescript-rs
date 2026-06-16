use crate::{
    new_fourslash, ApplyCodeActionFromCompletionOptions, CompletionsExpectedItem,
    CompletionsExpectedItemDefaults, CompletionsExpectedItems, CompletionsExpectedList,
    ExpectedCompletionEditRange, MarkerInput, TestingT,
};
use ts_lsproto as lsproto;

pub fn test_exhaustive_case_completions2(t: &mut TestingT) {
    let content = r#"// @newline: LF
// @Filename: /dep.ts
export enum E {
    A = 0,
    B = "B",
    C = "C",
}
declare const u: E.A | E.B | 1;
export { u };
// @Filename: /main.ts
import { u } from "./dep";
switch (u) {
    case/*1*/
}
// @Filename: /other.ts
import * as d from "./dep";
declare const u: d.E;
switch (u) {
    case/*2*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    verify_case_completion(
        &mut f,
        t,
        "1",
        "case 1: ...",
        "case 1:$1\ncase E.A:$2\ncase E.B:$3",
        true,
    );
    verify_case_completion(
        &mut f,
        t,
        "2",
        "case d.E.A: ...",
        "case d.E.A:$1\ncase d.E.B:$2\ncase d.E.C:$3",
        false,
    );
    f.verify_apply_code_action_from_completion(
        t,
        Some("1"),
        &ApplyCodeActionFromCompletionOptions {
            name: "case 1: ...".to_string(),
            source: "SwitchCases/".to_string(),
            auto_import_fix: None,
            description: String::new(),
            new_file_content: Some(
                r#"import { E, u } from "./dep";
switch (u) {
    case
}"#
                .to_string(),
            ),
            new_range_content: None,
            user_preferences: None,
        },
    );
    done();
}

fn verify_case_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker: &str,
    label: &str,
    insert_text: &str,
    has_additional_text_edits: bool,
) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![case_completion_item(
                label,
                insert_text,
                has_additional_text_edits,
            )],
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

fn case_completion_item(
    label: &str,
    insert_text: &str,
    has_additional_text_edits: bool,
) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.insert_text = Some(insert_text.to_string());
    item.sort_text = Some("15".to_string());
    item.insert_text_format = Some(lsproto::InsertTextFormat::Snippet);
    if has_additional_text_edits {
        item.additional_text_edits = Some(Vec::new());
    }
    CompletionsExpectedItem::Item(item)
}

