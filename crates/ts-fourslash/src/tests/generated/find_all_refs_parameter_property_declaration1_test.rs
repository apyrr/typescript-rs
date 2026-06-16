#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_parameter_property_declaration1() {
    let mut t = TestingT;
    run_test_find_all_refs_parameter_property_declaration1(&mut t);
}

fn run_test_find_all_refs_parameter_property_declaration1(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsParameterPropertyDeclaration1") {
        return;
    }
    let content = r"class Foo {
    constructor(private /*1*/privateParam: number) {
        let localPrivate = privateParam;
        this.privateParam += 10;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
