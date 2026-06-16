#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_parsing_dynamic_import3() {
    let mut t = TestingT;
    run_test_incremental_parsing_dynamic_import3(&mut t);
}

fn run_test_incremental_parsing_dynamic_import3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es2015
// @Filename: ./foo.ts
export function bar() { return 1; }
// @Filename: ./0.ts
var x = import/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "1");
    f.insert(t, "(");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
