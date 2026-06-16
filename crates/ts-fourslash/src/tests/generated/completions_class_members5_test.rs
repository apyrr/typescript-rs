#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_members5() {
    let mut t = TestingT;
    run_test_completions_class_members5(&mut t);
}

fn run_test_completions_class_members5(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsClassMembers5") {
        return;
    }
    let content = r"export const SOME_CONSTANT = 'SOME_TEXT';
export class Base {
    [SOME_CONSTANT]: boolean;
}
export class Derived extends Base {
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
