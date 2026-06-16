use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};

// Test for issue: Completions crash in call to `new Map(...)`.
// When requesting completions inside a Map constructor's array literal,
// IndexOfNode returns -1 causing a panic in getContextualTypeForElementExpression
// when trying to access array elements without a bounds check.
pub fn test_completions_in_map_constructor_no_crash(t: &mut TestingT) {
    // Test completion at position /*a*/ - before the string literal
    let content1 = r#"const m = new Map([
    [/*a*/'0', ['0', false]],
]);"#;
    let (mut f1, done1) = new_fourslash(t, None /*capabilities*/, content1.to_string());
    // Just verify that completions don't crash - accept any completion list
    verify_empty_completion(&mut f1, t, "a");
    done1();

    // Test completion at position /*b*/ - after the array literal
    let content2 = r#"const m = new Map([
    ['0', ['0', false]]/*b*/,
]);"#;
    let (mut f2, done2) = new_fourslash(t, None /*capabilities*/, content2.to_string());
    // Just verify that completions don't crash - accept any completion list
    verify_empty_completion(&mut f2, t, "b");
    done2();
}

fn verify_empty_completion(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: Vec::new(),
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

