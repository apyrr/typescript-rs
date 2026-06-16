#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_namespace_03() {
    let mut t = TestingT;
    run_test_go_to_implementation_namespace_03(&mut t);
}

fn run_test_go_to_implementation_namespace_03(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationNamespace_03") {
        return;
    }
    let content = r"namespace Foo {
    export interface Bar {
        hello(): void;
    }

    class [|BarImpl|] implements Bar {
        hello() {}
    }
}

class [|Baz|] implements Foo.Bar {
    hello() {}
}

var someVar1 : Foo.Bar = [|{ hello: () => {/**1*/} }|];

var someVar2 = <Foo.Bar> [|{ hello: () => {/**2*/} }|];

function whatever(x: Foo.Ba/*reference*/r) {

}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["reference".to_string()]);
    done();
}
