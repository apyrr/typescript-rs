#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_arguments_inside_function() {
    let mut t = TestingT;
    run_test_quick_info_on_arguments_inside_function(&mut t);
}

fn run_test_quick_info_on_arguments_inside_function(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo(x: string) {
    return /*1*/arguments;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(local var) arguments: IArguments", "");
    done();
}
