#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_implementation01() {
    let mut t = TestingT;
    run_test_implementation01(&mut t);
}

fn run_test_implementation01(t: &mut TestingT) {
    if should_skip_if_failing("TestImplementation01") {
        return;
    }
    let content = r"// @lib: es5
interface Fo/*1*/o {}
class /*2*/Bar implements Foo {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_implementation(t, &["1".to_string()]);
    done();
}
