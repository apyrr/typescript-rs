#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_alias() {
    let mut t = TestingT;
    run_test_go_to_definition_alias(&mut t);
}

fn run_test_go_to_definition_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionAlias") {
        return;
    }
    let content = r#"// @Filename: b.ts
import /*alias1Definition*/alias1 = require("fileb");
namespace Module {
    export import /*alias2Definition*/alias2 = alias1;
}

// Type position
var t1: [|/*alias1Type*/alias1|].IFoo;
var t2: Module.[|/*alias2Type*/alias2|].IFoo;

// Value posistion
var v1 = new [|/*alias1Value*/alias1|].Foo();
var v2 = new Module.[|/*alias2Value*/alias2|].Foo();
// @Filename: a.ts
export class Foo {
    private f;
}
export interface IFoo {
    x;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "alias1Type".to_string(),
            "alias1Value".to_string(),
            "alias2Type".to_string(),
            "alias2Value".to_string(),
        ],
    );
    done();
}
