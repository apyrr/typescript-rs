#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_expando_class2() {
    let mut t = TestingT;
    run_test_go_to_definition_expando_class2(&mut t);
}

fn run_test_go_to_definition_expando_class2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: true
// @allowJs: true
// @checkJs: true
// @filename: index.js
const Core = {}

Core.Test = class {
  constructor() { }
}

Core.Test.prototype.foo = 10

new Core.Tes/*1*/t()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
