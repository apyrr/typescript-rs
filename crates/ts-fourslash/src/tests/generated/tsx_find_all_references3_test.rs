#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_find_all_references3() {
    let mut t = TestingT;
    run_test_tsx_find_all_references3(&mut t);
}

fn run_test_tsx_find_all_references3(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxFindAllReferences3") {
        return;
    }
    let content = r"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
    }
    interface ElementAttributesProperty { props }
}
class MyClass {
  props: {
    /*1*/name?: string;
    size?: number;
}


var x = <MyClass name='hello'/>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
