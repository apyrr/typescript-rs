#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_class_method_01() {
    let mut t = TestingT;
    run_test_go_to_implementation_class_method_01(&mut t);
}

fn run_test_go_to_implementation_class_method_01(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationClassMethod_01") {
        return;
    }
    let content = r"abstract class AbstractBar {
    abstract he/*declaration*/llo(): void;
}

class Bar extends AbstractBar{
    [|hello|]() {}
}

function whatever(x: AbstractBar) {
    x.he/*reference*/llo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &["reference".to_string(), "declaration".to_string()],
    );
    done();
}
