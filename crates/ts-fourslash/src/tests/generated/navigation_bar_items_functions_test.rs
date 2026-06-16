#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_functions() {
    let mut t = TestingT;
    run_test_navigation_bar_items_functions(&mut t);
}

fn run_test_navigation_bar_items_functions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo() {
    var x = 10;
    function bar() {
        var y = 10;
        function biz() {
            var z = 10;
        }
        function qux() {
            // A function with an empty body should not be top level
        }
    }
}

function baz() {
    var v = 10;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
