#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_parameter_names() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_parameter_names(&mut t);
}

fn run_test_inlay_hints_interactive_parameter_names(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsInteractiveParameterNames") {
        return;
    }
    let content = r#" function foo1 (a: number, b: number) {}
 foo1(1, 2);
 function foo2 (a: number, { c }: any) {}
 foo2(1, { c: 1 });
const foo3 = (a = 1) => class { }
const C1 = class extends foo3(1) { }
class C2 extends foo3(1) { }
function foo4(a: number, b: number, c: number, d: number) {}
foo4(1, +1, -1, +"1");
function foo5(
    a: string,
    b: undefined,
    c: null,
    d: boolean,
    e: boolean,
    f: number,
    g: number,
    h: number,
    i: RegExp,
    j: bigint,
) {
}
foo5(
    "hello",
    undefined,
    null,
    true,
    false,
    Infinity,
    -Infinity,
    NaN,
    /hello/g,
    123n,
);
 declare const unknownCall: any;
 unknownCall();
function trace(message: string) {}
trace(`${1}`);
trace(``);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
