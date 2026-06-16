#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_completion14() {
    let mut t = TestingT;
    run_test_tsx_completion14(&mut t);
}

fn run_test_tsx_completion14(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@module: commonjs
//@jsx: preserve
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props; }
}
//@Filename: exporter.tsx
export class Thing { props: { ONE: string; TWO: number } }
export namespace M {
   export declare function SFCComp(props: { Three: number; Four: string }): JSX.Element;
}
//@Filename: file.tsx
import * as Exp from './exporter';
var x1 = <Exp.Thing /*1*/ />;
var x2 = <Exp.M.SFCComp /*2*/ />;
var x3 = <Exp.Thing /*3*/ ></Exp.Thing>;
var x4 = <Exp.M.SFCComp /*4*/ ></Exp.M.SFCComp>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "3".to_string()]),
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
                    CompletionsExpectedItem::Label("ONE".to_string()),
                    CompletionsExpectedItem::Label("TWO".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["2".to_string(), "4".to_string()]),
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
                    CompletionsExpectedItem::Label("Four".to_string()),
                    CompletionsExpectedItem::Label("Three".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
