#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_anonymous_class_and_function_expressions() {
    let mut t = TestingT;
    run_test_navigation_bar_anonymous_class_and_function_expressions(&mut t);
}

fn run_test_navigation_bar_anonymous_class_and_function_expressions(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarAnonymousClassAndFunctionExpressions") {
        return;
    }
    let content = r#"global.cls = class { };
(function() {
    const x = () => {
        // Presence of inner function causes x to be a top-level function.
        function xx() {}
    };
    const y = {
        // This is not a top-level function (contains nothing, but shows up in childItems of its parent.)
        foo: function() {}
    };
    (function nest() {
        function moreNest() {}
    })();
})();
(function() { // Different anonymous functions are not merged
    // These will only show up as childItems.
    function z() {}
    console.log(function() {})
    describe("this", 'function', `is a function`, `with template literal ${"a"}`, () => {});
    [].map(() => {});
})
(function classes() {
    // Classes show up in top-level regardless of whether they have names or inner declarations.
    const cls2 = class { };
    console.log(class cls3 {});
    (class { });
})"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
