#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_shorthand_property_assignment_00() {
    let mut t = TestingT;
    run_test_go_to_implementation_shorthand_property_assignment_00(&mut t);
}

fn run_test_go_to_implementation_shorthand_property_assignment_00(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo {
    someFunction(): void;
}

interface FooConstructor {
    new (): Foo
}

interface Bar {
    Foo: FooConstructor;
}

var x = class /*classExpression*/Foo {
    createBarInClassExpression(): Bar {
        return {
            Fo/*classExpressionRef*/o
        };
    }

    someFunction() {}
}

class /*declaredClass*/Foo {

}

function createBarUsingClassDeclaration(): Bar {
    return {
        Fo/*declaredClassRef*/o
    };
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &[
            "classExpressionRef".to_string(),
            "declaredClassRef".to_string(),
        ],
    );
    done();
}
