use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};

pub fn test_completion_in_array_literal_after_invalid_token1(t: &mut TestingT) {
    let content = r#"const pairs: Record<string, [string, string]> = {
  a: ["x",:/*m1*/]
};
"#;

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
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("m1".to_string()), Some(&expected));
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

