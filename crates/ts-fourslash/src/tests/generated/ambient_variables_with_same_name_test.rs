#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_ambient_variables_with_same_name() {
    let mut t = TestingT;
    run_test_ambient_variables_with_same_name(&mut t);
}

fn run_test_ambient_variables_with_same_name(t: &mut TestingT) {
    if should_skip_if_failing("TestAmbientVariablesWithSameName") {
        return;
    }
    let content = r"declare namespace M {
    export var x: string;
}
declare var x: number;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
    f.insert_line(t, "");
    f.verify_no_errors();
    done();
}
