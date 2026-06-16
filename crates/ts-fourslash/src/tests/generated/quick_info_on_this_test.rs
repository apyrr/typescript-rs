#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_this() {
    let mut t = TestingT;
    run_test_quick_info_on_this(&mut t);
}

fn run_test_quick_info_on_this(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnThis") {
        return;
    }
    let content = r"interface Restricted {
    n: number;
}
function wrapper(wrapped: { (): void; }) { }
class Foo {
    n: number;
    prop1: th/*0*/is;
    public explicitThis(this: this) {
        wrapper(
            function explicitVoid(this: void) {
                console.log(th/*1*/is);
            }
        )
        console.log(th/*2*/is);
    }
    public explicitInterface(th/*3*/is: Restricted) {
        console.log(th/*4*/is);
    }
    public explicitClass(th/*5*/is: Foo) {
        console.log(th/*6*/is);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "0", "this", "");
    f.verify_quick_info_at(t, "1", "this: void", "");
    f.verify_quick_info_at(t, "2", "this: this", "");
    f.verify_quick_info_at(t, "3", "(parameter) this: Restricted", "");
    f.verify_quick_info_at(t, "4", "this: Restricted", "");
    f.verify_quick_info_at(t, "5", "(parameter) this: Foo", "");
    f.verify_quick_info_at(t, "6", "this: Foo", "");
    done();
}
