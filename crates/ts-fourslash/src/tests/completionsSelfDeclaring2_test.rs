use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, EditRange, ExpectedCompletionEditRange,
    MarkerInput, TestingT,
};
use crate::tests::util::completion_globals_plus;

pub fn test_completions_self_declaring2(t: &mut TestingT) {
    let content = r#"// @lib: es5
function f1<T>(x: T) {}
f1({ [|abc|]/*1*/ });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(Vec::new()),
            edit_range: ExpectedCompletionEditRange::EditRange(EditRange {
                insert: ranges[0].clone(),
                replace: ranges[0].clone(),
            }),
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: completion_globals_plus(
                vec![CompletionsExpectedItem::Label("f1".to_string())],
                false,
            ),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("1".to_string()), Some(&expected));
    done();
}

