#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_shorthand_property_assignment_01() {
    let mut t = TestingT;
    run_test_go_to_implementation_shorthand_property_assignment_01(&mut t);
}

fn run_test_go_to_implementation_shorthand_property_assignment_01(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationShorthandPropertyAssignment_01") {
        return;
    }
    let content = r"interface Foo {
    someFunction(): void;
}

interface FooConstructor {
    new (): Foo
}

interface Bar {
    Foo: FooConstructor;
}

// Class expression that gets used in a bar implementation
var x = class [|Foo|] {
    createBarInClassExpression(): Bar {
        return {
            Foo
        };
    }

    someFunction() {}
};

// Class declaration that gets used in a bar implementation. This class has multiple definitions
// (the class declaration and the interface above), but we only want the class returned
class [|Foo|] {

}

function createBarUsingClassDeclaration(): Bar {
    return {
        Foo
    };
}

// Class expression that does not get used in a bar implementation
var y = class Foo {
    someFunction() {}
};

createBarUsingClassDeclaration().Fo/*reference*/o;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["reference".to_string()]);
    done();
}
