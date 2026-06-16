#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_ambient_class_implement_class_method_via_heritage() {
    let mut t = TestingT;
    run_test_code_fix_ambient_class_implement_class_method_via_heritage(&mut t);
}

fn run_test_code_fix_ambient_class_implement_class_method_via_heritage(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAmbientClassImplementClassMethodViaHeritage") {
        return;
    }
    let content = r"class C1 {
    f1() {}
}

class C2 extends C1 {

}

declare class C3 implements C2 {[|
    |]f2();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "f1(): void;\n", false, 0, 0);
    done();
}
