use crate::{
    new_fourslash, CompletionsExpectedItemDefaults, CompletionsExpectedItems,
    CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput, TestingT,
};
use crate::tests::util::completion_globals;

pub fn test_completion_colon_token(t: &mut TestingT) {
    let content = r#"
// @filename: /a.ts
:/*a*/

// @filename: /b.ts
function b(class: /*b*/) {}

// @filename: /c.ts
function c(enum: /*c*/) {}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    for marker in f.ranges() {
        f.verify_completions(
            t,
            MarkerInput::Range(marker),
            Some(&CompletionsExpectedList {
                is_incomplete: false,
                item_defaults: Some(CompletionsExpectedItemDefaults {
                    commit_characters: Some(default_commit_characters()),
                    edit_range: ExpectedCompletionEditRange::Ignored,
                }),
                items: Some(CompletionsExpectedItems {
                    includes: completion_globals(),
                    excludes: Vec::new(),
                    exact: Vec::new(),
                    unsorted: Vec::new(),
                }),
                user_preferences: None,
            }),
        );
    }
    done();
}

fn default_commit_characters() -> Vec<String> {
    [".", ",", ";", "("]
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

