#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_parsing_dynamic_import2() {
    let mut t = TestingT;
    run_test_incremental_parsing_dynamic_import2(&mut t);
}

fn run_test_incremental_parsing_dynamic_import2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es2015
// @Filename: ./foo.ts
export function bar() { return 1; }
// @Filename: ./0.ts
/*1*/ import { bar } from "./foo""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "1");
    f.insert(t, "var x = ");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
