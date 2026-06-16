#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_object_literal_properties1() {
    let mut t = TestingT;
    run_test_go_to_definition_object_literal_properties1(&mut t);
}

fn run_test_go_to_definition_object_literal_properties1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface PropsBag {
   /*first*/propx: number
}
function foo(arg: PropsBag) {}
foo({
   [|pr/*p1*/opx|]: 10
})
function bar(firstarg: boolean, secondarg: PropsBag) {}
bar(true, {
   [|pr/*p2*/opx|]: 10
})";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["p1".to_string(), "p2".to_string()]);
    done();
}
