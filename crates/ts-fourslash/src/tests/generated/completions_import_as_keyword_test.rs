#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_as_keyword() {
    let mut t = TestingT;
    run_test_completions_import_as_keyword(&mut t);
}

fn run_test_completions_import_as_keyword(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
// @Filename: /a.ts
export function as() {}
// @Filename: /b.ts
1 a/*1*/
a/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
