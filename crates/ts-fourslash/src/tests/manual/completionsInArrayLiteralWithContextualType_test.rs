use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};

// Test that string literal completions are suggested in tuple contexts
// even without typing a quote character.
pub fn test_completions_in_array_literal_with_contextual_type(t: &mut TestingT) {
    // Test 1: Completions after `[` in a tuple should suggest string literals
    let content1 = r#"let y: ["foo" | "bar", string] = [/*a*/];"#;
    let (mut f1, done1) = new_fourslash(t, None /*capabilities*/, content1.to_string());
    verify_includes(&mut f1, t, "a", &["\"foo\"", "\"bar\""]);
    done1();

    // Test 2: Completions after `,` in a tuple should provide contextual type for second element
    let content2 = r#"let z: ["a", "b" | "c"] = ["a", /*b*/];"#;
    let (mut f2, done2) = new_fourslash(t, None /*capabilities*/, content2.to_string());
    verify_includes(&mut f2, t, "b", &["\"b\"", "\"c\""]);
    done2();

    // Test 3: Verify that properties named "-1" are NOT suggested in array literals
    // This was a bug in the old implementation where passing -1 as an index would
    // check for a property named "-1" and suggest its value
    let content3 = r#"let x: { "-1": "hello" } = [/*c*/];"#;
    let (mut f3, done3) = new_fourslash(t, None /*capabilities*/, content3.to_string());
    verify_excludes(&mut f3, t, "c", &["\"hello\""]);
    done3();

    // Test 4: Completions after `]` in a tuple should not crash (issue #2296)
    // When completing after the closing bracket, we're outside the array literal
    // so we shouldn't be getting contextual types for array elements
    let content4 = r#"let x: [number] = [123]/*d*/;"#;
    let (mut f4, done4) = new_fourslash(t, None /*capabilities*/, content4.to_string());
    // Just verify that completions don't crash - accept any completion list
    verify_empty_items(&mut f4, t, "d");
    done4();
}

fn verify_includes(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str, labels: &[&str]) {
    let expected = expected_list(completion_items(labels), Vec::new());
    f.verify_completions(t, MarkerInput::Name(marker.to_string()), Some(&expected));
}

fn verify_excludes(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str, labels: &[&str]) {
    let expected = expected_list(Vec::new(), labels.iter().map(|value| (*value).to_string()).collect());
    f.verify_completions(t, MarkerInput::Name(marker.to_string()), Some(&expected));
}

fn verify_empty_items(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str) {
    let expected = expected_list(Vec::new(), Vec::new());
    f.verify_completions(t, MarkerInput::Name(marker.to_string()), Some(&expected));
}

fn expected_list(includes: Vec<CompletionsExpectedItem>, excludes: Vec<String>) -> CompletionsExpectedList {
    CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes,
            excludes,
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    }
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn completion_items(values: &[&str]) -> Vec<CompletionsExpectedItem> {
    values
        .iter()
        .map(|value| CompletionsExpectedItem::Label((*value).to_string()))
        .collect()
}

