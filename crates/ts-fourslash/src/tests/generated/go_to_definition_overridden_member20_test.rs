#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_overridden_member20() {
    let mut t = TestingT;
    run_test_go_to_definition_overridden_member20(&mut t);
}

fn run_test_go_to_definition_overridden_member20(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: true
// @target: esnext
// @lib: esnext
const prop = "foo" as const;

abstract class A {
  readonly /*2*/[prop] = "A";
}

export class B extends A {
  [|/*1*/override|] readonly [prop] = "B";
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
