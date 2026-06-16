#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_parameter_with_nested_destructuring() {
    let mut t = TestingT;
    run_test_parameter_with_nested_destructuring(&mut t);
}

fn run_test_parameter_with_nested_destructuring(t: &mut TestingT) {
    if should_skip_if_failing("TestParameterWithNestedDestructuring") {
        return;
    }
    let content = r"[[{ a: 'hello', b: [1] }]]
  .map(([{ a, b: [c] }]) => /*1*/a + /*2*/c);
function f([[/*3*/a]]: [[string]], { b1: { /*4*/b2 } }: { b1: { b2: string; } }) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "2", "(parameter) c: number", "");
    f.verify_quick_info_at(t, "3", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "4", "(parameter) b2: string", "");
    done();
}
