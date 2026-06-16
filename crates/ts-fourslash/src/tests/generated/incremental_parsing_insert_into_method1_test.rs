#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_parsing_insert_into_method1() {
    let mut t = TestingT;
    run_test_incremental_parsing_insert_into_method1(&mut t);
}

fn run_test_incremental_parsing_insert_into_method1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    public foo1() { }
    public foo2() {
        return 1/*1*/;
    }
    public foo3() { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, " + 1");
    done();
}
