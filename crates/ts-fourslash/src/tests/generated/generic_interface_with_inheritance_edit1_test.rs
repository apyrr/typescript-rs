#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_interface_with_inheritance_edit1() {
    let mut t = TestingT;
    run_test_generic_interface_with_inheritance_edit1(&mut t);
}

fn run_test_generic_interface_with_inheritance_edit1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface ChainedObject<T> {
    values(): ChainedArray<any>;
    pairs(): ChainedArray<any[]>;
    extend(...sources: any[]): ChainedObject<T>;

    value(): T;
}
interface ChainedArray<T> extends ChainedObject<Array<T>> {

    extend(...sources: any[]): ChainedArray<T>;
}
 /*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.insert(t, " ");
    f.verify_no_errors();
    done();
}
