#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_extends_t_array() {
    let mut t = TestingT;
    run_test_extends_t_array(&mut t);
}

fn run_test_extends_t_array(t: &mut TestingT) {
    if should_skip_if_failing("TestExtendsTArray") {
        return;
    }
    let content = r"// @strict: false
interface I1<T> {
    (a: T): T;
}
interface I2<T> extends I1<T[]> {
    b: T;
}
var x: I2<Date>;
var /**/y = x(undefined); // Typeof y should be Date[]
y.length;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var y: Date[]", "");
    f.verify_no_errors();
    done();
}
