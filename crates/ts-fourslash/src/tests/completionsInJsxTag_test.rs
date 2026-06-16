use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completions_in_jsx_tag(t: &mut TestingT) {
    let content = r#"// @jsx: preserve
// @Filename: /a.tsx
declare namespace JSX {
    interface Element {}
    interface IntrinsicElements {
        div: {
            /** Doc */
            foo: string
            /** Label docs */
            "aria-label": string
        }
    }
}
class Foo {
    render() {
        <div /*1*/ ></div>;
        <div  /*2*/ />
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::None,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: vec![
                CompletionsExpectedItem::Item(completion_item(
                    "aria-label",
                    "(property) \"aria-label\": string",
                    "Label docs",
                )),
                CompletionsExpectedItem::Item(completion_item("foo", "(property) foo: string", "Doc")),
            ],
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "2".to_string()]),
        Some(&expected),
    );
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn completion_item(label: &str, detail: &str, documentation: &str) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.kind = Some(lsproto::CompletionItemKind::Field);
    item.detail = Some(detail.to_string());
    item.documentation = Some(lsproto::StringOrMarkupContent {
        markup_content: Some(lsproto::MarkupContent {
            kind: lsproto::MarkupKind::Markdown,
            value: documentation.to_string(),
        }),
        ..Default::default()
    });
    item
}

