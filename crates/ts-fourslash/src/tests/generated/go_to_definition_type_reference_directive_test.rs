#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_type_reference_directive() {
    let mut t = TestingT;
    run_test_go_to_definition_type_reference_directive(&mut t);
}

fn run_test_go_to_definition_type_reference_directive(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionTypeReferenceDirective") {
        return;
    }
    let content = r#"// @typeRoots: src/types
// @Filename: src/types/lib/index.d.ts
/*0*/declare let $: {x: number};
// @Filename: src/app.ts
 /// <reference types="[|lib/*1*/|]"/>
 $.x;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
