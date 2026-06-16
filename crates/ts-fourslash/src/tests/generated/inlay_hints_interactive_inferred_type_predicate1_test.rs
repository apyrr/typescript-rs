#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_inferred_type_predicate1() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_inferred_type_predicate1(&mut t);
}

fn run_test_inlay_hints_interactive_inferred_type_predicate1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: true
function test(x: unknown) {
  return typeof x === 'number';
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
