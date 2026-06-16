use crate::{
    new_fourslash, skip_if_failing, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};

pub fn test_string_literal_completions_in_position_typed_using_rest(t: &mut TestingT) {
    skip_if_failing("TestStringLiteralCompletionsInPositionTypedUsingRest");
    let content = r#"declare function pick<T extends object, K extends keyof T>(obj: T, ...keys: K[]): Pick<T, K>;
declare function pick2<T extends object, K extends (keyof T)[]>(obj: T, ...keys: K): Pick<T, K[number]>;

const obj = { aaa: 1, bbb: '2', ccc: true };

pick(obj, 'aaa', '/*ts1*/');
pick2(obj, 'aaa', '/*ts2*/');
class Q<T> {
  public select<Keys extends keyof T>(...args: Keys[]) {}
}
new Q<{ id: string; name: string }>().select("name", "/*ts3*/");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts1".to_string(), "ts2".to_string()]),
        Some(&expected_exact(&["aaa", "bbb", "ccc"])),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts3".to_string()]),
        Some(&expected_exact(&["id", "name"])),
    );
    done();
}

fn expected_exact(labels: &[&str]) -> CompletionsExpectedList {
    CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: Vec::new(),
            excludes: Vec::new(),
            exact: labels
                .iter()
                .map(|label| CompletionsExpectedItem::Label((*label).to_string()))
                .collect(),
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

