#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_param_suggestion1() {
    let mut t = TestingT;
    run_test_jsdoc_param_suggestion1(&mut t);
}

fn run_test_jsdoc_param_suggestion1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocParam_suggestion1") {
        return;
    }
    let content = r"// @Filename: a.ts
/**
 * @param options - whatever
 * @param options.zone - equally bad
 */
declare function bad(options: any): void

/**
 * @param {number} obtuse
 */
function worse(): void {
    arguments
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "a.ts");
    f.verify_suggestion_diagnostics(&[]);
    done();
}
