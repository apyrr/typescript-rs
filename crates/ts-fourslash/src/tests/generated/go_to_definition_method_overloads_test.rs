#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_method_overloads() {
    let mut t = TestingT;
    run_test_go_to_definition_method_overloads(&mut t);
}

fn run_test_go_to_definition_method_overloads(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionMethodOverloads") {
        return;
    }
    let content = r#"class MethodOverload {
    static [|/*staticMethodOverload1*/method|]();
    static /*staticMethodOverload2*/method(foo: string);
    static /*staticMethodDefinition*/method(foo?: any) { }
    public [|/*instanceMethodOverload1*/method|](): any;
    public /*instanceMethodOverload2*/method(foo: string);
    public /*instanceMethodDefinition*/method(foo?: any) { return "foo" }
}
// static method
MethodOverload.[|/*staticMethodReference1*/method|]();
MethodOverload.[|/*staticMethodReference2*/method|]("123");
// instance method
var methodOverload = new MethodOverload();
methodOverload.[|/*instanceMethodReference1*/method|]();
methodOverload.[|/*instanceMethodReference2*/method|]("456");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "staticMethodReference1".to_string(),
            "staticMethodReference2".to_string(),
            "instanceMethodReference1".to_string(),
            "instanceMethodReference2".to_string(),
            "staticMethodOverload1".to_string(),
            "instanceMethodOverload1".to_string(),
        ],
    );
    done();
}
