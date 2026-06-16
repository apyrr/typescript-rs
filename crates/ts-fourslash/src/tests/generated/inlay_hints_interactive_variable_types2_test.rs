#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_variable_types2() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_variable_types2(&mut t);
}

fn run_test_inlay_hints_interactive_variable_types2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const object = { foo: 1, bar: 2 }
const array = [1, 2]
const a = object;
const { foo, bar } = object;
const {} = object;
const b = array;
const [ first, second ] = array;
const [] = array;
declare function foo<T extends number>(t: T): T
const x = foo(1)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
