#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_variables() {
    let mut t = TestingT;
    run_test_navigation_bar_variables(&mut t);
}

fn run_test_navigation_bar_variables(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x = 0;
let y = 1;
const z = 2;
// @Filename: file2.ts
var {a} = 0;
let {a: b} = 0;
const [c] = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    f.go_to_file(t, "file2.ts");
    f.verify_baseline_document_symbol(t);
    done();
}
