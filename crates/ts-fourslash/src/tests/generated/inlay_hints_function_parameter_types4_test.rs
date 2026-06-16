#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_function_parameter_types4() {
    let mut t = TestingT;
    run_test_inlay_hints_function_parameter_types4(&mut t);
}

fn run_test_inlay_hints_function_parameter_types4(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsFunctionParameterTypes4") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: /a.js
class Foo {
    #value = 0;
    get foo() { return this.#value; }
    /**
     * @param {number} value
     */
    set foo(value) { this.#value = value; }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
