use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_resolve_keyword(t: &mut TestingT) {
    let content = r#"class C {
	/*a*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(Vec::new()),
            edit_range: ExpectedCompletionEditRange::None,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(abstract_completion_item())],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("a".to_string()), Some(&expected));
    done();
}

fn abstract_completion_item() -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "abstract".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Keyword);
    item.sort_text = Some("15".to_string());
    item.detail = Some("abstract".to_string());
    item
}

