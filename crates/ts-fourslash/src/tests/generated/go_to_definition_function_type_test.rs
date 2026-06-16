#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_function_type() {
    let mut t = TestingT;
    run_test_go_to_definition_function_type(&mut t);
}

fn run_test_go_to_definition_function_type(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionFunctionType") {
        return;
    }
    let content = r"const /*constDefinition*/c: () => void;
/*constReference*/c();
function test(/*cbDefinition*/cb: () => void) {
    /*cbReference*/cb();
}
class C {
    /*propDefinition*/prop: () => void;
    m() {
        this./*propReference*/prop();
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "constReference".to_string(),
            "cbReference".to_string(),
            "propReference".to_string(),
        ],
    );
    done();
}
