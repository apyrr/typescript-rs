#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_of_constructor_bad_overload() {
    let mut t = TestingT;
    run_test_find_all_references_of_constructor_bad_overload(&mut t);
}

fn run_test_find_all_references_of_constructor_bad_overload(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesOfConstructor_badOverload") {
        return;
    }
    let content = r"class C {
    /*1*/constructor(n: number);
    /*2*/constructor(){}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
