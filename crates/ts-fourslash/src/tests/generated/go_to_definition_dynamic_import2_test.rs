#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_dynamic_import2() {
    let mut t = TestingT;
    run_test_go_to_definition_dynamic_import2(&mut t);
}

fn run_test_go_to_definition_dynamic_import2(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionDynamicImport2") {
        return;
    }
    let content = r#"// @Filename: foo.ts
export function /*Destination*/bar() { return "bar"; }
var x = import("./foo");
x.then(foo => {
    foo.[|b/*1*/ar|](); 
})"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
