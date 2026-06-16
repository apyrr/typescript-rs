#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_of_a_fundule() {
    let mut t = TestingT;
    run_test_type_of_a_fundule(&mut t);
}

fn run_test_type_of_a_fundule(t: &mut TestingT) {
    if should_skip_if_failing("TestTypeOfAFundule") {
        return;
    }
    let content = r"function m1() { return 1; }
namespace m1 { export var y = 2; }
function foo13() {
    return m1;
}
var /**/r13 = foo13();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var r13: typeof m1", "");
    done();
}
