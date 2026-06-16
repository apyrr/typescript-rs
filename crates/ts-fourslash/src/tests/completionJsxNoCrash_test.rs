use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};

pub fn test_completion_jsx_no_crash(t: &mut TestingT) {
    let content = r#"
// @filename: file.tsx
<Foo/>/*1*/
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // The assertion here is simply "does not crash/panic".
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some([".", ",", ";"].into_iter().map(|value| value.to_string()).collect()),
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
    f.verify_completions(t, MarkerInput::Name("1".to_string()), Some(&expected));
    done();
}

