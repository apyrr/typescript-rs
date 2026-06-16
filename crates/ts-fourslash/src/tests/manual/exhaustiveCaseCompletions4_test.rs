use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use crate::tests::util::completion_globals_plus;
use ts_lsproto as lsproto;

// Filter existing values.
pub fn test_exhaustive_case_completions4(t: &mut TestingT) {
    let content = r#"// @lib: es5
// @newline: LF
enum E {
    A = 0,
    B = "B",
    C = "C",
}
// Filtering existing literals
declare const u: E.A | E.B | 1 | 1n | "1";
switch (u) {
    case E.A:
    case 1:
    case 1n:
    case 0x1n:
    case "1":
    case `1`:
    case `1${u}`:
    case/*1*/
}
declare const v: E.A | "1" | "2";
switch (v) {
    case 0:
    case `1`:
    /*2*/
}
// Filtering repeated enum members
enum F {
    A = "A",
    B = "B",
    C = A,
}
declare const x: F;
switch (x) {
    /*3*/
}
// Enum with computed elements
enum G {
    C = 0,
    D = 1 << 1,
    E = 1 << 2,
    OtherD = D,
    DorE = D | E,
}
declare const y: G;
switch (y) {
    /*4*/
}
switch (y) {
    case 0: // same as G.C
    case 1: // same as G.D, but we don't know it
    case 3: // same as G.DorE, but we don't know
    /*5*/
}

// Already exhaustive switch
enum H {
    A = "A",
    B = "B",
    C = "C",
}
declare const z: H;
switch (z) {
    case H.A:
    case H.B:
    case H.C:
    /*6*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    verify_case_completion(&mut f, t, "1", "case E.B: ...", "case E.B:$1");
    verify_case_completion(&mut f, t, "2", "case \"2\": ...", "case \"2\":$1");
    // F.A and F.B (no C because C's value is the same as A's)
    verify_case_completion(&mut f, t, "3", "case F.A: ...", "case F.A:$1\ncase F.B:$2");
    verify_case_completion(
        &mut f,
        t,
        "4",
        "case G.C: ...",
        "case G.C:$1\ncase G.D:$2\ncase G.E:$3\ncase G.DorE:$4",
    );
    verify_case_completion(
        &mut f,
        t,
        "5",
        "case G.D: ...",
        "case G.D:$1\ncase G.E:$2\ncase G.DorE:$3",
    );

    // No exhaustive case completion offered here because the switch is already exhaustive
    let expected = expected_list(
        Vec::new(),
        completion_globals_plus(completion_items(&["E", "F", "G", "H", "u", "v", "x", "y", "z"]), false),
    );
    f.verify_completions(t, MarkerInput::Name("6".to_string()), Some(&expected));
    done();
}

fn verify_case_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker: &str,
    label: &str,
    insert_text: &str,
) {
    let expected = expected_list(vec![case_completion_item(label, insert_text)], Vec::new());
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

fn case_completion_item(label: &str, insert_text: &str) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.insert_text = Some(insert_text.to_string());
    item.sort_text = Some("15".to_string());
    item.insert_text_format = Some(lsproto::InsertTextFormat::Snippet);
    CompletionsExpectedItem::Item(item)
}

fn completion_items(values: &[&str]) -> Vec<CompletionsExpectedItem> {
    values
        .iter()
        .map(|value| CompletionsExpectedItem::Label((*value).to_string()))
        .collect()
}

