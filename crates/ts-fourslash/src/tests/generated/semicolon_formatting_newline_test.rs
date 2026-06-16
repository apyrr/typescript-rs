#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semicolon_formatting_newline() {
    let mut t = TestingT;
    run_test_semicolon_formatting_newline(&mut t);
}

fn run_test_semicolon_formatting_newline(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare var f: { 
    (): any;
    (x: number): string;
            foo: number;/**/
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    done();
}
