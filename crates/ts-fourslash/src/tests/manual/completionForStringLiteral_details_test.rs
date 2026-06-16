use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_for_string_literal_details(t: &mut TestingT) {
    let content = r#"// @Filename: /other.ts
export const x = 0;
// @Filename: /a.ts
import {} from ".//*path*/";

const x: "a" = "[|/*type*/|]";

interface I {
    /** Prop doc */
    x: number;
    /** Method doc */
    m(): void;
}
declare const o: I;
o["[|/*prop*/|]"];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    verify_completion(
        &mut f,
        t,
        "path",
        Vec::new(),
        vec![completion_item(
            "other",
            "other.ts",
            lsproto::CompletionItemKind::File,
            None,
            None,
        )],
        Some(Vec::new()),
    );
    verify_completion(
        &mut f,
        t,
        "type",
        vec![completion_item(
            "a",
            "a",
            lsproto::CompletionItemKind::Constant,
            None,
            Some(text_edit("a", ranges[0].ls_range.clone())),
        )],
        Vec::new(),
        Some(default_commit_characters()),
    );
    verify_completion(
        &mut f,
        t,
        "prop",
        vec![
            completion_item(
                "m",
                "(method) I.m(): void",
                lsproto::CompletionItemKind::Method,
                Some("Method doc"),
                Some(text_edit("m", ranges[1].ls_range.clone())),
            ),
            completion_item(
                "x",
                "(property) I.x: number",
                lsproto::CompletionItemKind::Field,
                Some("Prop doc"),
                Some(text_edit("x", ranges[1].ls_range.clone())),
            ),
        ],
        Vec::new(),
        Some(default_commit_characters()),
    );
    done();
}

fn verify_completion(
    f: &mut crate::FourslashTest,
    t: &mut TestingT,
    marker: &str,
    exact: Vec<CompletionsExpectedItem>,
    includes: Vec<CompletionsExpectedItem>,
    commit_characters: Option<Vec<String>>,
) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters,
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes,
            excludes: Vec::new(),
            exact,
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

fn completion_item(
    label: &str,
    detail: &str,
    kind: lsproto::CompletionItemKind,
    documentation: Option<&str>,
    text_edit: Option<lsproto::TextEditOrInsertReplaceEdit>,
) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.detail = Some(detail.to_string());
    item.kind = Some(kind);
    item.text_edit = text_edit;
    if let Some(documentation) = documentation {
        item.documentation = Some(lsproto::StringOrMarkupContent {
            markup_content: Some(lsproto::MarkupContent {
                kind: lsproto::MarkupKind::Markdown,
                value: documentation.to_string(),
            }),
            ..Default::default()
        });
    }
    CompletionsExpectedItem::Item(item)
}

fn text_edit(new_text: &str, range: lsproto::Range) -> lsproto::TextEditOrInsertReplaceEdit {
    lsproto::TextEditOrInsertReplaceEdit::TextEdit(lsproto::TextEdit {
        new_text: new_text.to_string(),
        range,
    })
}

