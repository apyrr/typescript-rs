#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_recursive_namespace() {
    let mut t = TestingT;
    run_test_completions_recursive_namespace(&mut t);
}

fn run_test_completions_recursive_namespace(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare namespace N {
    export import M = N;
}
type T = N./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
