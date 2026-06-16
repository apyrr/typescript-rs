#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_overloads_in_multiple_property_accesses() {
    let mut t = TestingT;
    run_test_go_to_definition_overloads_in_multiple_property_accesses(&mut t);
}

fn run_test_go_to_definition_overloads_in_multiple_property_accesses(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace A {
    export namespace B {
        export function f(value: number): void;
        export function /*1*/f(value: string): void;
        export function f(value: number | string) {}
    }
}
A.B.[|/*2*/f|]("");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["2".to_string()]);
    done();
}
