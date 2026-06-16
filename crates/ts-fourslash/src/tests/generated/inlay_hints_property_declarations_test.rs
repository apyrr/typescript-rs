#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_property_declarations() {
    let mut t = TestingT;
    run_test_inlay_hints_property_declarations(&mut t);
}

fn run_test_inlay_hints_property_declarations(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsPropertyDeclarations") {
        return;
    }
    let content = r"// @strict: true
class C {
    a = 1
    b: number = 2
    c;
    d;

    constructor(value: number) {
        this.d = value;
        if (value <= 0) {
            this.d = null;
        }
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
