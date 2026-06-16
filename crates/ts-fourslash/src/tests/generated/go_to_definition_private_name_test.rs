#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_private_name() {
    let mut t = TestingT;
    run_test_go_to_definition_private_name(&mut t);
}

fn run_test_go_to_definition_private_name(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionPrivateName") {
        return;
    }
    let content = r#"class A {
    [|/*pnMethodDecl*/#method|]() { }
    [|/*pnFieldDecl*/#foo|] = 3;
    get [|/*pnPropGetDecl*/#prop|]() { return ""; }
    set [|/*pnPropSetDecl*/#prop|](value: string) {  }
    constructor() {
        this.[|/*pnFieldUse*/#foo|]
        this.[|/*pnMethodUse*/#method|]
        this.[|/*pnPropUse*/#prop|]
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "pnFieldUse".to_string(),
            "pnMethodUse".to_string(),
            "pnPropUse".to_string(),
        ],
    );
    done();
}
