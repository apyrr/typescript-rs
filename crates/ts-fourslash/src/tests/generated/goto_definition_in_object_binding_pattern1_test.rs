#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_goto_definition_in_object_binding_pattern1() {
    let mut t = TestingT;
    run_test_goto_definition_in_object_binding_pattern1(&mut t);
}

fn run_test_goto_definition_in_object_binding_pattern1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function bar<T>(onfulfilled: (value: T) => void) {
  return undefined;
}
interface Test {
  /*destination*/prop2: number
}
bar<Test>(({[|pr/*goto*/op2|]})=>{});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["goto".to_string()]);
    done();
}
