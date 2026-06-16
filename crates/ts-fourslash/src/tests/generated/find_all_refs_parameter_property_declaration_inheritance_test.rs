#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_parameter_property_declaration_inheritance() {
    let mut t = TestingT;
    run_test_find_all_refs_parameter_property_declaration_inheritance(&mut t);
}

fn run_test_find_all_refs_parameter_property_declaration_inheritance(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
	constructor(public /*0*/x: string) {
		/*1*/x;
	}
}
class D extends C {
	constructor(public /*2*/x: string) {
		super(/*3*/x);
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
        ],
    );
    done();
}
