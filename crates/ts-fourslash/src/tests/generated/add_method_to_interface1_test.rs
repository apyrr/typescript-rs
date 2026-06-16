#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_method_to_interface1() {
    let mut t = TestingT;
    run_test_add_method_to_interface1(&mut t);
}

fn run_test_add_method_to_interface1(t: &mut TestingT) {
    if should_skip_if_failing("TestAddMethodToInterface1") {
        return;
    }
    let content = r"interface Comparable<T> {
/*1*/}
interface Comparer {
}
var max2: Comparer = (x, y) => { return (x.compareTo(y) > 0) ? x : y };
var maxResult = max2(1);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.insert(t, "    compareTo(): number;\n");
    done();
}
