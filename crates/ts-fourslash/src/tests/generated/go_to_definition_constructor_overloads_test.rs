#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_constructor_overloads() {
    let mut t = TestingT;
    run_test_go_to_definition_constructor_overloads(&mut t);
}

fn run_test_go_to_definition_constructor_overloads(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionConstructorOverloads") {
        return;
    }
    let content = r#"class ConstructorOverload {
    [|/*constructorOverload1*/constructor|]();
    /*constructorOverload2*/constructor(foo: string);
    /*constructorDefinition*/constructor(foo: any)  { }
}

var constructorOverload = new [|/*constructorOverloadReference1*/ConstructorOverload|]();
var constructorOverload = new [|/*constructorOverloadReference2*/ConstructorOverload|]("foo");

class Extended extends ConstructorOverload {
    readonly name = "extended";
}
var extended1 = new [|/*extendedRef1*/Extended|]();
var extended2 = new [|/*extendedRef2*/Extended|]("foo");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "constructorOverloadReference1".to_string(),
            "constructorOverloadReference2".to_string(),
            "constructorOverload1".to_string(),
            "extendedRef1".to_string(),
            "extendedRef2".to_string(),
        ],
    );
    done();
}
