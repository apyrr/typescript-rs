#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_01() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_01(&mut t);
}

fn run_test_go_to_implementation_interface_01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Fo/*interface_definition*/o { hello(): void }

class [|SuperBar|] implements Foo {
    hello () {}
}

abstract class [|AbstractBar|] implements Foo {
    abstract hello (): void;
}

class [|Bar|] extends SuperBar {
}

class [|NotAbstractBar|] extends AbstractBar {
    hello () {}
}

var x = new SuperBar();
var y: SuperBar = new SuperBar();
var z: AbstractBar = new NotAbstractBar();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["interface_definition".to_string()]);
    done();
}
