use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};
use crate::tests::util::completion_global_types;

pub fn test_completion_list_default_type_argument_position_type_only(t: &mut TestingT) {
    let content = r#"// @lib: es5
const foo = "foo";
function test1<T = /*1*/>() {}"#;
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
            exact: completion_global_types(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("1".to_string()), Some(&expected));
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

