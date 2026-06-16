#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_object_literal_properties2() {
    let mut t = TestingT;
    run_test_go_to_definition_object_literal_properties2(&mut t);
}

fn run_test_go_to_definition_object_literal_properties2(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionObjectLiteralProperties2") {
        return;
    }
    let content = r#"type C = {
  foo: string;
  bar: number;
};

declare function fn<T extends C>(arg: T): T;

fn({
  foo/*1*/: "",
  bar/*2*/: true,
});

const result = fn({
  foo/*3*/: "",
  bar/*4*/: 1,
});

// this one shouldn't go to the constraint type
result.foo/*5*/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ],
    );
    done();
}
