use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};

pub fn test_completion_list_in_unclosed_type_arguments(t: &mut TestingT) {
    let content = r#"let x = 10;
type Type = void;
declare function f<T>(): void;
declare function f2<T, U>(): void;
f</*1a*/T/*2a*/y/*3a*/
f</*1b*/T/*2b*/y/*3b*/;
f</*1c*/T/*2c*/y/*3c*/>
f</*1d*/T/*2d*/y/*3d*/>
f</*1e*/T/*2e*/y/*3e*/>();

f2</*1k*/T/*2k*/y/*3k*/,
f2</*1l*/T/*2l*/y/*3l*/,{| "newId": true |}T{| "newId": true |}y{| "newId": true |}
f2</*1m*/T/*2m*/y/*3m*/,{| "newId": true |}T{| "newId": true |}y{| "newId": true |};
f2</*1n*/T/*2n*/y/*3n*/,{| "newId": false |}T{| "newId": false |}y{| "newId": false |}>
f2</*1o*/T/*2o*/y/*3o*/,{| "newId": false |}T{| "newId": false |}y{| "newId": false |}>
f2</*1p*/T/*2p*/y/*3p*/,{| "newId": true, "typeOnly": true |}T{| "newId": true, "typeOnly": true |}y{| "newId": true, "typeOnly": true |}>();

f2<typeof /*1uValueOnly*/x, {| "newId": true |}T{| "newId": true |}y{| "newId": true |}

f2</*1x*/T/*2x*/y/*3x*/, () =>/*4x*/T/*5x*/y/*6x*/
f2<() =>/*1y*/T/*2y*/y/*3y*/, () =>/*4y*/T/*5y*/y/*6y*/
f2<any, () =>/*1z*/T/*2z*/y/*3z*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.markers() {
        let marker_name = marker.name.clone();
        let value_only = marker_name
            .as_deref()
            .map(|name| name.ends_with("ValueOnly"))
            .unwrap_or(false);
        let new_id = marker.data.get("newId").map(|value| value == "true").unwrap_or(false);
        let type_only = marker.data.get("typeOnly").map(|value| value == "true").unwrap_or(false);
        let commit_characters = if new_id && !type_only {
            [".", ";"].into_iter().map(|value| value.to_string()).collect()
        } else {
            default_commit_characters()
        };
        let (includes, excludes) = if value_only {
            (completion_items(&["x"]), vec!["Type".to_string()])
        } else {
            (completion_items(&["Type"]), vec!["x".to_string()])
        };
        let expected = CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(commit_characters),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes,
                excludes,
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        };
        f.verify_completions(t, MarkerInput::Marker(marker), Some(&expected));
    }
    done();
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

