#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal16() {
    let mut t = TestingT;
    run_test_completion_for_string_literal16(&mut t);
}

fn run_test_completion_for_string_literal16(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Foo {
    a: string;
    b: number;
    c: string;
}

declare function f1<T>(key: keyof T): T;
declare function f2<T>(a: keyof T, b: keyof T): T;

f1<Foo>("/*1*/",);
f1<Foo>("/*2*/");
f1<Foo>("/*3*/",,,);
f2<Foo>("/*4*/", "/*5*/",);
f2<Foo>("/*6*/", "/*7*/");
f2<Foo>("/*8*/", "/*9*/",,,);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("c".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
