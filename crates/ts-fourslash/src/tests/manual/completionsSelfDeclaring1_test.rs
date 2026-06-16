use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completions_self_declaring1(t: &mut TestingT) {
    let content = r#"interface Test {
  keyPath?: string;
  autoIncrement?: boolean;
}

function test<T extends Record<string, Test>>(opt: T) { }

test({
  a: {
    keyPath: '',
    [|a|]/**/
  }
})"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let range = f.ranges()[0].ls_range.clone();
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: vec![CompletionsExpectedItem::Item(auto_increment_completion_item(range))],
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&expected));
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn auto_increment_completion_item(range: lsproto::Range) -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "autoIncrement?".to_string();
    item.filter_text = Some("autoIncrement".to_string());
    item.sort_text = Some("12".to_string());
    item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit::InsertReplaceEdit(
        lsproto::InsertReplaceEdit {
            new_text: "autoIncrement".to_string(),
            insert: range.clone(),
            replace: range,
        },
    ));
    item
}

