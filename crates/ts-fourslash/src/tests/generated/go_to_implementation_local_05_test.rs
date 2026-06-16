#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_local_05() {
    let mut t = TestingT;
    run_test_go_to_implementation_local_05(&mut t);
}

fn run_test_go_to_implementation_local_05(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationLocal_05") {
        return;
    }
    let content = r"class Bar {
    public hello() {}
}

var [|someVar|] = new Bar();
someVa/*reference*/r.hello();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["reference".to_string()]);
    done();
}
