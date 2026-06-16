#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_type_predicate() {
    let mut t = TestingT;
    run_test_go_to_definition_type_predicate(&mut t);
}

fn run_test_go_to_definition_type_predicate(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionTypePredicate") {
        return;
    }
    let content = r#"class /*classDeclaration*/A {}
function f(/*parameterDeclaration*/parameter: any): [|/*parameterName*/parameter|] is [|/*typeReference*/A|] {
    return typeof parameter === "string";
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &["parameterName".to_string(), "typeReference".to_string()],
    );
    done();
}
