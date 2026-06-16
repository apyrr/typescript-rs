#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_is_definition_overloads() {
    let mut t = TestingT;
    run_test_is_definition_overloads(&mut t);
}

fn run_test_is_definition_overloads(t: &mut TestingT) {
    if should_skip_if_failing("TestIsDefinitionOverloads") {
        return;
    }
    let content = r#"function /*1*/f(x: number): void;
function /*2*/f(x: string): void;
function /*3*/f(x: number | string) { }

f(1);
f("a");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
