#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_union_string_literal_property() {
    let mut t = TestingT;
    run_test_completions_union_string_literal_property(&mut t);
}

fn run_test_completions_union_string_literal_property(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsUnionStringLiteralProperty") {
        return;
    }
    let content = r"type Foo = { a: 0, b: 'x' } | { a: 0, b: 'y' } | { a: 1, b: 'z' };
const foo: Foo = { a: 0, b: '/*1*/' }

type Bar = { a: 0, b: 'fx' } | { a: 0, b: 'fy' } | { a: 1, b: 'fz' };
const bar: Bar = { a: 0, b: 'f/*2*/' }

type Baz = { x: 0, y: 0, z: 'a' } | { x: 0, y: 1, z: 'b' } | { x: 1, y: 0, z: 'c' } | { x: 1, y: 1, z: 'd' };
const baz1: Baz = { z: '/*3*/' };
const baz2: Baz = { x: 0, z: '/*4*/' };
const baz3: Baz = { x: 0, y: 1, z: '/*5*/' };
const baz4: Baz = { x: 2, y: 1, z: '/*6*/' };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                ],
                excludes: vec!["z".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("fx".to_string()),
                    CompletionsExpectedItem::Label("fy".to_string()),
                ],
                excludes: vec!["fz".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("3".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("c".to_string()),
                    CompletionsExpectedItem::Label("d".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("4".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                ],
                excludes: vec!["c".to_string(), "d".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("5".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("b".to_string())],
                excludes: vec!["a".to_string(), "c".to_string(), "d".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("6".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("d".to_string()),
                ],
                excludes: vec!["a".to_string(), "c".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
