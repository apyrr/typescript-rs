#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_commit_characters_after_dot() {
    let mut t = TestingT;
    run_test_completions_commit_characters_after_dot(&mut t);
}

fn run_test_completions_commit_characters_after_dot(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
declare const obj: { banana: 1 };
const x = obj./*1*/
declare module obj./*2*/ {}
declare const obj2: { banana: 1 } | undefined;
const y = obj2?./*3*/
declare const obj3: { [x: string]: number };
const z = obj3./*4*/
declare const obj4: { (): string; [x: string]: number } | undefined;
const w = obj4?./*5*/
declare const obj5: { (): string } | undefined;
const a = obj5?./*6*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
