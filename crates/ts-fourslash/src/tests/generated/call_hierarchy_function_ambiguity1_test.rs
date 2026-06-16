#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_function_ambiguity1() {
    let mut t = TestingT;
    run_test_call_hierarchy_function_ambiguity1(&mut t);
}

fn run_test_call_hierarchy_function_ambiguity1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @filename: a.d.ts
declare function foo(x?: number): void;
// @filename: b.d.ts
declare function foo(x?: string): void;
declare function foo(x?: boolean): void;
// @filename: main.ts
function bar() {
    /**/foo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
