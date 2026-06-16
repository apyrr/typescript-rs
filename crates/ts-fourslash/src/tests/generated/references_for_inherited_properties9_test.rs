#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_inherited_properties9() {
    let mut t = TestingT;
    run_test_references_for_inherited_properties9(&mut t);
}

fn run_test_references_for_inherited_properties9(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class D extends C {
    /*1*/prop1: string;
}

class C extends D {
    /*2*/prop1: string;
}

var c: C;
c./*3*/prop1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
