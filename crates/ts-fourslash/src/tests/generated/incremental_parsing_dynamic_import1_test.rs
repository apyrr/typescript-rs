#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_parsing_dynamic_import1() {
    let mut t = TestingT;
    run_test_incremental_parsing_dynamic_import1(&mut t);
}

fn run_test_incremental_parsing_dynamic_import1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es6
// @module: commonjs
// @Filename: ./foo.ts
export function bar() { return 1; }
var x1 = import("./foo");
x1.then(foo => {
   var s: string = foo.bar();
})
/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "1");
    f.insert(t, "  ");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
