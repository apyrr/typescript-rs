use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

// Test exhaustive case completions for locally defined enum in untitled file.
pub fn test_exhaustive_case_completions_untitled_local_enum(t: &mut TestingT) {
    let content = r#"// @newline: LF
// @filename: ^/untitled/ts-nul-authority/Untitled-1.ts
enum E {
    A = "A",
    B = "B",
    C = "C",
}
declare const e: E;
switch (e) {
    case/**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    // Locally defined enum should provide exhaustive case completions in untitled file
    verify_case_completion(&mut f, t, "case E.A: ...", "case E.A:$1\ncase E.B:$2\ncase E.C:$3");
    done();
}

// Test exhaustive case completions for globally declared enum in untitled file.
pub fn test_exhaustive_case_completions_untitled_global_enum(t: &mut TestingT) {
    let content = r#"// @newline: LF
// @filename: /home/src/project/globals.d.ts
declare enum Direction {
	Up = "Up",
	Down = "Down",
	Left = "Left",
	Right = "Right",
}
declare const direction: Direction;

// @filename: ^/untitled/ts-nul-authority/Untitled-1.ts
switch (direction) {
    case/**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    // Globally declared enum should provide exhaustive case completions in untitled file
    verify_case_completion(
        &mut f,
        t,
        "case Direction.Up: ...",
        "case Direction.Up:$1\ncase Direction.Down:$2\ncase Direction.Left:$3\ncase Direction.Right:$4",
    );
    done();
}

// Test exhaustive case completions for string literal union in untitled file.
pub fn test_exhaustive_case_completions_untitled_string_literals(t: &mut TestingT) {
    let content = r#"// @newline: LF
// @filename: ^/untitled/ts-nul-authority/Untitled-1.ts
export {};
declare const status: "pending" | "success" | "error";
switch (status) {
    case/**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    // String literal unions should provide exhaustive case completions in untitled file
    verify_case_completion(
        &mut f,
        t,
        "case \"error\": ...",
        "case \"error\":$1\ncase \"pending\":$2\ncase \"success\":$3",
    );
    done();
}

// Test that imported enum type reference doesn't crash.
// Turns out the easiest way to do this is to provide the completions
// without associated auto-import edits, which is a pretty nice UX anyway.
pub fn test_exhaustive_case_completions_untitled_imported_enum(t: &mut TestingT) {
    let content = r#"// @newline: LF
// @filename: /home/src/project/enums.ts
export enum Status {
    Active,
    Inactive,
    Pending,
}

// @filename: ^/untitled/ts-nul-authority/Untitled-1.ts
declare const s: import("/home/src/project/enums").Status;
switch (s) {
    case/**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    verify_case_completion(
        &mut f,
        t,
        "case Status.Active: ...",
        "case Status.Active:$1\ncase Status.Inactive:$2\ncase Status.Pending:$3",
    );
    done();
}

fn verify_case_completion(f: &mut crate::FourslashTest, t: &mut TestingT, label: &str, insert_text: &str) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(case_completion_item(label, insert_text))],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&expected));
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn case_completion_item(label: &str, insert_text: &str) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.insert_text = Some(insert_text.to_string());
    item.sort_text = Some("15".to_string());
    item.insert_text_format = Some(lsproto::InsertTextFormat::Snippet);
    item
}

