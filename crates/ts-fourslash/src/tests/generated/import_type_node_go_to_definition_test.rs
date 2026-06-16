#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_type_node_go_to_definition() {
    let mut t = TestingT;
    run_test_import_type_node_go_to_definition(&mut t);
}

fn run_test_import_type_node_go_to_definition(t: &mut TestingT) {
    if should_skip_if_failing("TestImportTypeNodeGoToDefinition") {
        return;
    }
    let content = r#"// @Filename: /ns.ts
/*refFile*/export namespace /*refFoo*/Foo {
    export namespace /*refBar*/Bar {
        export class /*refBaz*/Baz {}
    }
}
// @Filename: /usage.ts
type A = typeof import([|/*1*/"./ns"|]).[|/*2*/Foo|].[|/*3*/Bar|];
type B = import([|/*4*/"./ns"|]).[|/*5*/Foo|].[|/*6*/Bar|].[|/*7*/Baz|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
        ],
    );
    done();
}
