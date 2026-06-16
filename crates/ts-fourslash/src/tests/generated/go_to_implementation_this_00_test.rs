#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_this_00() {
    let mut t = TestingT;
    run_test_go_to_implementation_this_00(&mut t);
}

fn run_test_go_to_implementation_this_00(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationThis_00") {
        return;
    }
    let content = r"class [|Bar|] extends Foo {
    hello() {
        thi/*this_call*/s.whatever();
    }

    whatever() {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["this_call".to_string()]);
    done();
}
