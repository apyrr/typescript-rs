#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_index_signature_without_annotation() {
    let mut t = TestingT;
    run_test_index_signature_without_annotation(&mut t);
}

fn run_test_index_signature_without_annotation(t: &mut TestingT) {
    if should_skip_if_failing("TestIndexSignatureWithoutAnnotation") {
        return;
    }
    let content = r"interface B {
    1: any;
}
interface C {
    [s]: any;
}
interface D extends B, C /**/ {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, " ");
    done();
}
