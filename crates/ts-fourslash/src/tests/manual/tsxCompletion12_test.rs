use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_tsx_completion12(t: &mut TestingT) {
    let content = r#"//@Filename: file.tsx
// @jsx: preserve
// @noLib: true
declare module JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props; }
}
interface OptionPropBag {
    propx: number
    propString: "hell"
    optional?: boolean
}
declare function Opt(attributes: OptionPropBag): JSX.Element;
let opt = <Opt /*1*/ />;
let opt1 = <Opt [|prop|]/*2*/ />;
let opt2 = <Opt propx={100} /*3*/ />;
let opt3 = <Opt propx={100} optional /*4*/ />;
let opt4 = <Opt wrong /*5*/ />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let replacement_range = f.ranges()[0].ls_range;
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "5".to_string()]),
        Some(&expected_exact(vec![
            label_item("propString"),
            label_item("propx"),
            optional_completion_item(true, None),
        ])),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&expected_exact(vec![
            label_item("propString"),
            label_item("propx"),
            optional_completion_item(false, Some(replacement_range)),
        ])),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("3".to_string()),
        Some(&expected_exact(vec![
            label_item("propString"),
            optional_completion_item(true, None),
        ])),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("4".to_string()),
        Some(&expected_exact(vec![label_item("propString")])),
    );
    done();
}

fn expected_exact(exact: Vec<CompletionsExpectedItem>) -> CompletionsExpectedList {
    CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact,
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    }
}

fn label_item(label: &str) -> CompletionsExpectedItem {
    CompletionsExpectedItem::Label(label.to_string())
}

fn optional_completion_item(with_insert_text: bool, replacement_range: Option<lsproto::Range>) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "optional?".to_string();
    if with_insert_text {
        item.insert_text = Some("optional".to_string());
    }
    item.filter_text = Some("optional".to_string());
    item.kind = Some(lsproto::CompletionItemKind::Field);
    item.sort_text = Some("12".to_string());
    if let Some(range) = replacement_range {
        item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::InsertReplaceEdit(
            lsproto::InsertReplaceEdit {
                new_text: "optional".to_string(),
                insert: range,
                replace: range,
            },
        ));
    }
    CompletionsExpectedItem::Item(item)
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

