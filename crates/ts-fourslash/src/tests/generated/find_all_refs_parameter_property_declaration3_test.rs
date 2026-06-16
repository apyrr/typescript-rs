#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_parameter_property_declaration3() {
    let mut t = TestingT;
    run_test_find_all_refs_parameter_property_declaration3(&mut t);
}

fn run_test_find_all_refs_parameter_property_declaration3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Foo {
    constructor(protected /*0*/protectedParam: number) {
        let localProtected = /*1*/protectedParam;
        this./*2*/protectedParam += 10;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
