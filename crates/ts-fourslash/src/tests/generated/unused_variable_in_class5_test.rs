#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_variable_in_class5() {
    let mut t = TestingT;
    run_test_unused_variable_in_class5(&mut t);
}

fn run_test_unused_variable_in_class5(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedVariableInClass5") {
        return;
    }
    let content = r"// @noUnusedLocals: true
// @target: esnext
declare class greeter {
    #private;
    private name;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    done();
}
