use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};

pub fn test_completions_from_untitled_file(t: &mut TestingT) {
    // Test that completions work in untitled files without crashing.
    // Regression test for https://github.com/microsoft/typescript-go/issues/2550
    let content = r#"// @filename: /home/src/project/utils.ts
export function helper() {}

// @filename: ^/untitled/ts-nul-authority/Untitled-1.ts
/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Request completions - this should not crash
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::None,
        }),
        items: Some(CompletionsExpectedItems {
            // We don't care about the exact completions, just that it doesn't crash
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: Vec::new(),
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

