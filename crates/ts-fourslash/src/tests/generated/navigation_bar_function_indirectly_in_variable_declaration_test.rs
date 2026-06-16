#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_function_indirectly_in_variable_declaration() {
    let mut t = TestingT;
    run_test_navigation_bar_function_indirectly_in_variable_declaration(&mut t);
}

fn run_test_navigation_bar_function_indirectly_in_variable_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var a = {
    propA: function() {
        var c;
    }
};
var b;
b = {
    propB: function() {
    // function must not have an empty body to appear top level
        var d;
    }
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
