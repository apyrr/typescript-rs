#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_await3() {
    let mut t = TestingT;
    run_test_go_to_definition_await3(&mut t);
}

fn run_test_go_to_definition_await3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    notAsync() {
      [|/*start1*/await|] Promise.resolve(0);
    }

    async /*end2*/foo() {
      [|/*start2*/await|] Promise.resolve(0);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["start1".to_string(), "start2".to_string()]);
    done();
}
