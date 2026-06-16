#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_namespace_05() {
    let mut t = TestingT;
    run_test_go_to_implementation_namespace_05(&mut t);
}

fn run_test_go_to_implementation_namespace_05(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace /*implementation0*/Foo./*implementation2*/Baz {
    export function hello() {}
}

module /*implementation1*/Bar./*implementation3*/Baz {
    export function sure() {}
}

let x = Fo/*reference0*/o;
let y = Ba/*reference1*/r;
let x1 = Foo.B/*reference2*/az;
let y1 = Bar.B/*reference3*/az;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &[
            "reference0".to_string(),
            "reference1".to_string(),
            "reference2".to_string(),
            "reference3".to_string(),
        ],
    );
    done();
}
