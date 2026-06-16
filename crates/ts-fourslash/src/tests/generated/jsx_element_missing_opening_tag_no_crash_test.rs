#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_element_missing_opening_tag_no_crash() {
    let mut t = TestingT;
    run_test_jsx_element_missing_opening_tag_no_crash(&mut t);
}

fn run_test_jsx_element_missing_opening_tag_no_crash(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
declare function Foo(): any;
let x = <></Fo/*$*/o>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "$", "let Foo: any", "");
    done();
}
