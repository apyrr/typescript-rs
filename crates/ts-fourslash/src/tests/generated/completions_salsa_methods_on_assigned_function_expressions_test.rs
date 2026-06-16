#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_salsa_methods_on_assigned_function_expressions() {
    let mut t = TestingT;
    run_test_completions_salsa_methods_on_assigned_function_expressions(&mut t);
}

fn run_test_completions_salsa_methods_on_assigned_function_expressions(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsSalsaMethodsOnAssignedFunctionExpressions") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: something.js
var C = function () { }
/**
 * The prototype method.
 * @param {string} a Parameter definition.
 */
function f(a) {}
C.prototype.m = f;

var x = new C();
x./*2*/m();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
