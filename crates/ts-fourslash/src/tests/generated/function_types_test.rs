#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_function_types() {
    let mut t = TestingT;
    run_test_function_types(&mut t);
}

fn run_test_function_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
// @strict: false
var f: Function;
function g() { }

class C {
    h: () => void ;
    i(): number { return 5; }
    static j = (e) => e;
    static k() { return 'hi';}
}
var l = () => void 0;
var z = new C;

f./*1*/apply(this, [1]);
g./*2*/arguments;
z.h./*3*/bind(undefined, 1, 2);
z.i./*4*/call(null)
C.j./*5*/length === 1;
typeof C.k./*6*/caller === 'function';
l./*7*/prototype = Object.prototype;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_function_members_with_prototype(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("7".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_function_members_plus(vec![CompletionsExpectedItem::Label(
                    "prototype".to_string(),
                )]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
