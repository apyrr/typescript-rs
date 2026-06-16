#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_external_module_name6() {
    let mut t = TestingT;
    run_test_go_to_definition_external_module_name6(&mut t);
}

fn run_test_go_to_definition_external_module_name6(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionExternalModuleName6") {
        return;
    }
    let content = r#"// @Filename: b.ts
import * from [|'e/*1*/'|];
// @Filename: a.ts
declare module /*2*/"e" {
    class Foo { }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
