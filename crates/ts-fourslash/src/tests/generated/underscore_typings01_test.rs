#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_underscore_typings01() {
    let mut t = TestingT;
    run_test_underscore_typings01(&mut t);
}

fn run_test_underscore_typings01(t: &mut TestingT) {
    if should_skip_if_failing("TestUnderscoreTypings01") {
        return;
    }
    let content = r"interface Iterator_<T, U> {
    (value: T, index: any, list: any): U;
}

interface WrappedArray<T> {
    map<U>(iterator: Iterator_<T, U>, context?: any): U[];
}

interface Underscore {
    <T>(list: T[]): WrappedArray<T>;
    map<T, U>(list: T[], iterator: Iterator_<T, U>, context?: any): U[];
}

declare var _: Underscore;

var a: string[];
var /*1*/b = _.map(a, /*2*/x => x.length);    // Was typed any[], should be number[]
var /*3*/c = _(a).map(/*4*/x => x.length);
var /*5*/d = a.map(/*6*/x => x.length);

var aa: any[];
var /*7*/bb = _.map(aa, /*8*/x => x.length);
var /*9*/cc = _(aa).map(/*10*/x => x.length);
var /*11*/dd = aa.map(/*12*/x => x.length);

var e = a.map(x => x./*13*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var b: number[]", "");
    f.verify_quick_info_at(t, "2", "(parameter) x: string", "");
    f.verify_quick_info_at(t, "3", "var c: number[]", "");
    f.verify_quick_info_at(t, "4", "(parameter) x: string", "");
    f.verify_quick_info_at(t, "5", "var d: number[]", "");
    f.verify_quick_info_at(t, "6", "(parameter) x: string", "");
    f.verify_quick_info_at(t, "7", "var bb: any[]", "");
    f.verify_quick_info_at(t, "8", "(parameter) x: any", "");
    f.verify_quick_info_at(t, "9", "var cc: any[]", "");
    f.verify_quick_info_at(t, "10", "(parameter) x: any", "");
    f.verify_quick_info_at(t, "11", "var dd: any[]", "");
    f.verify_quick_info_at(t, "12", "(parameter) x: any", "");
    f.verify_completions(
        t,
        MarkerInput::Name("13".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("length".to_string())],
                excludes: vec!["toFixed".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
