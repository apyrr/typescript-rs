#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_this4() {
    let mut t = TestingT;
    run_test_quick_info_on_this4(&mut t);
}

fn run_test_quick_info_on_this4(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnThis4") {
        return;
    }
    let content = r"interface ContextualInterface {
    m: number;
    method(this: this, n: number);
}
let o: ContextualInterface = {
    m: 12,
    method(n) {
        let x = this/*1*/.m;
    }
}
interface ContextualInterface2 {
    (this: void, n: number): void;
}
let contextualInterface2: ContextualInterface2 = function (th/*2*/is, n) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "this: ContextualInterface", "");
    f.verify_quick_info_at(t, "2", "(parameter) this: void", "");
    done();
}
