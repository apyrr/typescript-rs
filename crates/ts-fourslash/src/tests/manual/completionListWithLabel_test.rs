use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};

pub fn test_completion_list_with_label(t: &mut TestingT) {
    let content = r#" label: while (true) {
    break /*1*/
    continue /*2*/
    testlabel: while (true) {
        break /*3*/
        continue /*4*/
        break tes/*5*/
        continue tes/*6*/
    }
    break /*7*/
    break; /*8*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    verify_exact(&mut f, t, MarkerInput::Names(vec!["1".to_string(), "2".to_string(), "7".to_string()]), &["label"]);
    verify_exact(
        &mut f,
        t,
        MarkerInput::Names(vec!["3".to_string(), "4".to_string(), "5".to_string(), "6".to_string()]),
        &["label", "testlabel"],
    );
    verify_excludes(&mut f, t, MarkerInput::Name("8".to_string()), &["label"]);
    done();
}

fn verify_exact(f: &mut crate::FourslashTest, t: &mut TestingT, marker_input: MarkerInput, labels: &[&str]) {
    let expected = expected_list(Vec::new(), Vec::new(), completion_items(labels));
    f.verify_completions(t, marker_input, Some(&expected));
}

fn verify_excludes(f: &mut crate::FourslashTest, t: &mut TestingT, marker_input: MarkerInput, labels: &[&str]) {
    let expected = expected_list(Vec::new(), labels.iter().map(|value| (*value).to_string()).collect(), Vec::new());
    f.verify_completions(t, marker_input, Some(&expected));
}

fn expected_list(
    includes: Vec<CompletionsExpectedItem>,
    excludes: Vec<String>,
    exact: Vec<CompletionsExpectedItem>,
) -> CompletionsExpectedList {
    CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes,
            excludes,
            exact,
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

