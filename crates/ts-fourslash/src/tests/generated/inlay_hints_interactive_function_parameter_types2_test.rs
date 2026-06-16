#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_function_parameter_types2() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_function_parameter_types2(&mut t);
}

fn run_test_inlay_hints_interactive_function_parameter_types2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class C {}
namespace N { export class Foo {} }
interface Foo {}
function f1(a = 1) {}
function f2(a = "a") {}
function f3(a = true) {}
function f4(a = { } as Foo) {}
function f5(a = <Foo>{}) {}
function f6(a = {} as const) {}
function f7(a = (({} as const))) {}
function f8(a = new C()) {}
function f9(a = new N.C()) {}
function f10(a = ((((new C()))))) {}
function f11(a = { a: 1, b: 1 }) {}
function f12(a = ((({ a: 1, b: 1 })))) {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
