#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_inherited_properties5() {
    let mut t = TestingT;
    run_test_find_all_refs_inherited_properties5(&mut t);
}

fn run_test_find_all_refs_inherited_properties5(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsInheritedProperties5") {
        return;
    }
    let content = r"class C extends D {
    /*0*/prop0: string;
    /*1*/prop1: number;
}

class D extends C {
    /*2*/prop0: string;
}

var d: D;
d./*3*/prop0;
d./*4*/prop1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
