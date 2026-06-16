#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_function_overloads_in_class() {
    let mut t = TestingT;
    run_test_go_to_definition_function_overloads_in_class(&mut t);
}

fn run_test_go_to_definition_function_overloads_in_class(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionFunctionOverloadsInClass") {
        return;
    }
    let content = r#"class clsInOverload {
    static fnOverload();
    static [|/*staticFunctionOverload*/fnOverload|](foo: string);
    static /*staticFunctionOverloadDefinition*/fnOverload(foo: any) { }
    public [|/*functionOverload*/fnOverload|](): any;
    public fnOverload(foo: string);
    public /*functionOverloadDefinition*/fnOverload(foo: any) { return "foo" }

    constructor() { }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "staticFunctionOverload".to_string(),
            "functionOverload".to_string(),
        ],
    );
    done();
}
