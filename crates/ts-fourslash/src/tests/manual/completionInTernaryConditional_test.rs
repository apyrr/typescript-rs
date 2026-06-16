use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};
use ts_lsproto as lsproto;

pub fn test_completion_in_ternary_conditional(t: &mut TestingT) {
    let content = r#"export enum Bar { }
export enum Foo { }


function foo(x: Foo) { return x; }
function bar(z: string, x: Foo) { return x; }

const a = '';

foo(/*1*/);
bar(a, a == '' ? /*2*/);
bar(a, a == '' ? /*3*/ : /*4*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Test marker 1 - should have Foo preselected in simple call
    verify_foo_completion(&mut f, t, "1");

    // Test marker 2 - should have Foo preselected after ? in incomplete ternary
    verify_foo_completion(&mut f, t, "2");

    // Test marker 3 - should have Foo preselected after ? in ternary with colon
    verify_foo_completion(&mut f, t, "3");

    // Test marker 4 - should have Foo preselected after : in ternary
    verify_foo_completion(&mut f, t, "4");
    done();
}

fn verify_foo_completion(f: &mut crate::FourslashTest, t: &mut TestingT, marker: &str) {
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(default_commit_characters()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Item(foo_completion_item())],
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

fn foo_completion_item() -> lsproto::CompletionItem {
    let mut item = lsproto::CompletionItem::default();
    item.label = "Foo".to_string();
    item.kind = Some(lsproto::CompletionItemKind::Enum);
    item.preselect = Some(true);
    item
}

