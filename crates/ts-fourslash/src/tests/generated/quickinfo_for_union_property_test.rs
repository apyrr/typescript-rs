#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_for_union_property() {
    let mut t = TestingT;
    run_test_quickinfo_for_union_property(&mut t);
}

fn run_test_quickinfo_for_union_property(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface One {
    commonProperty: number;
    commonFunction(): number;
}

interface Two {
    commonProperty: string
    commonFunction(): number;
}

var /*1*/x : One | Two;

x./*2*/commonProperty;
x./*3*/commonFunction;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var x: One | Two", "");
    f.verify_quick_info_at(t, "2", "(property) commonProperty: string | number", "");
    f.verify_quick_info_at(t, "3", "(method) commonFunction(): number", "");
    done();
}
