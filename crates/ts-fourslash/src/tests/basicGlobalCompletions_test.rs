use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};
use crate::tests::util::completion_globals;

pub fn test_basic_global_completions(t: &mut TestingT) {
    let content = r#"// @lib: es5
/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::None,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

