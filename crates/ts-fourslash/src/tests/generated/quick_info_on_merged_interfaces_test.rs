#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_merged_interfaces() {
    let mut t = TestingT;
    run_test_quick_info_on_merged_interfaces(&mut t);
}

fn run_test_quick_info_on_merged_interfaces(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace M {
    interface A<T> {
        (): string;
        (x: T): T;
    }
    interface A<T> {
        (x: T, y: number): T;
        <U>(x: U, y: T): U;
    }
    var a: A<boolean>;
    var r = a();
    var r2 = a(true);
    var r3 = a(true, 2);
    var /*1*/r4 = a(1, true);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var r4: number", "");
    done();
}
