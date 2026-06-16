#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_call_expression_by_const_named_function_expression() {
    let mut t = TestingT;
    run_test_call_hierarchy_call_expression_by_const_named_function_expression(&mut t);
}

fn run_test_call_hierarchy_call_expression_by_const_named_function_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestCallHierarchyCallExpressionByConstNamedFunctionExpression") {
        return;
    }
    let content = r"function foo() {
    bar();
}

const bar = function () {
    baz();
}

function baz() {
}

/**/bar()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
