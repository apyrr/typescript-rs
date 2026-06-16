#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_inaccessible_types() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_inaccessible_types(&mut t);
}

fn run_test_code_fix_infer_from_usage_inaccessible_types(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixInferFromUsageInaccessibleTypes") {
        return;
    }
    let content = r"// @strict: false
// @noImplicitAny: true
function f1(a) { a; }
function h1() {
    class C { p: number };
    f1({ ofTypeC: new C() });
}

function f2(a) { a; }
function h2() {
    interface I { a: number }
    var i: I = {a : 1};
    f2(i);
    f2(2);
    f2(false);
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
