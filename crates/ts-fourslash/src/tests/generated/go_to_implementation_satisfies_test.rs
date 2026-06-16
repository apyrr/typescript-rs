#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_satisfies() {
    let mut t = TestingT;
    run_test_go_to_implementation_satisfies(&mut t);
}

fn run_test_go_to_implementation_satisfies(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementation_satisfies") {
        return;
    }
    let content = r"// @filename: /a.ts
interface /*def*/I {
	foo: string;
}

function f() {
    const foo = { foo: '' } satisfies [|I|];
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["def".to_string()]);
    done();
}
