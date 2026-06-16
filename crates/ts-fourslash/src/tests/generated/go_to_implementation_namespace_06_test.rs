#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_namespace_06() {
    let mut t = TestingT;
    run_test_go_to_implementation_namespace_06(&mut t);
}

fn run_test_go_to_implementation_namespace_06(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace [|F/*declaration*/oo|] {
    declare function hello(): void;
}


let x: typeof Foo = [|{ hello() {} }|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["declaration".to_string()]);
    done();
}
