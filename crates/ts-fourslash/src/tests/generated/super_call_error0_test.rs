#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_super_call_error0() {
    let mut t = TestingT;
    run_test_super_call_error0(&mut t);
}

fn run_test_super_call_error0(t: &mut TestingT) {
    if should_skip_if_failing("TestSuperCallError0") {
        return;
    }
    let content = r"class T5<T>{
    constructor(public bar: T) { }
}
class T6 extends T5<number>{
    constructor() {
        super();
    }
}/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "/n");
    done();
}
