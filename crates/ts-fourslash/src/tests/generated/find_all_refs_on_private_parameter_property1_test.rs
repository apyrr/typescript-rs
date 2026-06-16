#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_on_private_parameter_property1() {
    let mut t = TestingT;
    run_test_find_all_refs_on_private_parameter_property1(&mut t);
}

fn run_test_find_all_refs_on_private_parameter_property1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class ABCD {
    constructor(private x: number, public y: number, /*1*/private /*2*/z: number) {
    }

    func() {
        return this./*3*/z;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
