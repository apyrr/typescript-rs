#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextually_typed_object_literal_method_declaration_param01() {
    let mut t = TestingT;
    run_test_contextually_typed_object_literal_method_declaration_param01(&mut t);
}

fn run_test_contextually_typed_object_literal_method_declaration_param01(t: &mut TestingT) {
    if should_skip_if_failing("TestContextuallyTypedObjectLiteralMethodDeclarationParam01") {
        return;
    }
    let content = r#"// @noImplicitAny: true
interface A {
    numProp: number;
}

interface B  {
    strProp: string;
}

interface Foo {
    method1(arg: A): void;
    method2(arg: B): void;
}

function getFoo1(): Foo {
    return {
        method1(/*param1*/arg) {
            arg.numProp = 10;
        },
        method2(/*param2*/arg) {
            arg.strProp = "hello";
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "param1", "(parameter) arg: A", "");
    f.verify_quick_info_at(t, "param2", "(parameter) arg: B", "");
    done();
}
