#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_constructor_of_class_when_class_is_preceded_by_namespace01() {
    let mut t = TestingT;
    run_test_go_to_definition_constructor_of_class_when_class_is_preceded_by_namespace01(&mut t);
}

fn run_test_go_to_definition_constructor_of_class_when_class_is_preceded_by_namespace01(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"namespace Foo {
    export var x;
}

class Foo {
    /*definition*/constructor() {
    }
}

var x = new [|/*usage*/Foo|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["usage".to_string()]);
    done();
}
