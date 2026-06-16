#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_variable_assignment2() {
    let mut t = TestingT;
    run_test_go_to_definition_variable_assignment2(&mut t);
}

fn run_test_go_to_definition_variable_assignment2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @filename: foo.ts
const Bar;
const Foo = /*def*/Bar = function () {}
Foo.prototype.bar = function() {}
new [|Foo/*ref*/|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "foo.ts");
    f.verify_baseline_go_to_definition(t, &["ref".to_string()]);
    done();
}
