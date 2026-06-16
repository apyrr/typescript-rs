#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_circular_types() {
    let mut t = TestingT;
    run_test_quick_info_on_circular_types(&mut t);
}

fn run_test_quick_info_on_circular_types(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnCircularTypes") {
        return;
    }
    let content = r"interface A { (): B; };
declare var a: A;
var xx = a();

interface B { (): C; };
declare var b: B;
var yy = b();

interface C { (): A; };
declare var c: C;
var zz = c();

x/*B*/x = y/*C*/y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "B", "var xx: B", "");
    f.verify_quick_info_at(t, "C", "var yy: C", "");
    done();
}
