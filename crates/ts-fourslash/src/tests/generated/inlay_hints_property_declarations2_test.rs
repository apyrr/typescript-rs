#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_property_declarations2() {
    let mut t = TestingT;
    run_test_inlay_hints_property_declarations2(&mut t);
}

fn run_test_inlay_hints_property_declarations2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: true
// @target: esnext
class C {
    accessor a = 1
    accessor b: number = 2
    accessor c;
    accessor d;

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
