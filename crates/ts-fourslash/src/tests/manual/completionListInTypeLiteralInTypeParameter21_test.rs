use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};

pub fn test_completion_list_in_type_literal_in_type_parameter21(t: &mut TestingT) {
    let content = r#"class Foo<T extends ('one' | 2)[]> {}
function foo<T extends ('one' | 2)[]>() {}

type A = Foo<[/*0*/]>;
new Foo<[/*1*/]>();
foo<[/*2*/]>();
foo<[/*3*/]>;
Foo<[/*4*/]>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in ["0", "1", "2", "3", "4"] {
        verify_literal_completions(&mut f, t, marker);
    }
    done();
}

fn verify_literal_completions(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: completion_items(&["\"one\"", "2"]),
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

fn completion_items(values: &[&str]) -> Vec<CompletionsExpectedItem> {
    values
        .iter()
        .map(|value| CompletionsExpectedItem::Label((*value).to_string()))
        .collect()
}

