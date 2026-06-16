#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_union_type_property3() {
    let mut t = TestingT;
    run_test_go_to_definition_union_type_property3(&mut t);
}

fn run_test_go_to_definition_union_type_property3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Array<T> {
    /*definition*/specialPop(): T
}

var strings: string[];
var numbers: number[];

var x = (strings || numbers).[|/*usage*/specialPop|]()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["usage".to_string()]);
    done();
}
