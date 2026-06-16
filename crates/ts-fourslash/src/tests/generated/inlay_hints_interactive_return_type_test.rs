#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_return_type() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_return_type(&mut t);
}

fn run_test_inlay_hints_interactive_return_type(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsInteractiveReturnType") {
        return;
    }
    let content = r"function foo1 () {
    return 1
}
function foo2 (): number {
    return 1
}
class C {
    foo() {
        return 1
    }
    bar() {
        return this
    }
}
const a = () => 1
const b = function () { return 1 }
const c = (b) => 1
const d = b => 1";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
