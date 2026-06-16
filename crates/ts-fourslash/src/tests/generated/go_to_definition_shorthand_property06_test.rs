#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_shorthand_property06() {
    let mut t = TestingT;
    run_test_go_to_definition_shorthand_property06(&mut t);
}

fn run_test_go_to_definition_shorthand_property06(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionShorthandProperty06") {
        return;
    }
    let content = r"interface Foo {
    /*2*/foo(): void
}
const foo = 1;
let x: Foo = {
    [|f/*1*/oo|]()
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
