#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_export_equals_expression() {
    let mut t = TestingT;
    run_test_navigation_items_export_equals_expression(&mut t);
}

fn run_test_navigation_items_export_equals_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"export = function () {}
export = function () {
    return class Foo {
    }
}

export = () => ""
export = () => {
    return class Foo {
    }
}

export = function f1() {}
export = function f2() {
    return class Foo {
    }
}

const abc = 12;
export = abc;
export = class AB {}
export = {
    a: 1,
    b: 1,
    c: {
        d: 1
    }
}

function foo(props: { x: number; y: number }) {}
export = foo({ x: 1, y: 1 });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
