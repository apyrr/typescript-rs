use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT, UserPreferences,
};
use ts_lsproto as lsproto;

pub fn test_completion_import_module_specifier_ending_jsx(t: &mut TestingT) {
    let content = r#"//@allowJs: true
//@jsx:preserve
//@Filename:test.jsx
 export class Test { }
//@Filename:module.jsx
import { Test } from ".//**/""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    verify_completion(&mut f, t, "test.jsx", "js");
    verify_completion(&mut f, t, "test", "index");
    done();
}

fn verify_completion(f: &mut crate::FourslashTest, t: &mut TestingT, label: &str, ending: &str) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(Vec::new()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![completion_item(label, "test.jsx")],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: Some(UserPreferences {
            import_module_specifier_ending: Some(ending.to_string()),
            ..UserPreferences::default()
        }),
    };
    f.verify_completions(t, MarkerInput::Name("".to_string()), Some(&expected));
}

fn completion_item(label: &str, detail: &str) -> CompletionsExpectedItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = label.to_string();
    item.detail = Some(detail.to_string());
    CompletionsExpectedItem::Item(item)
}

