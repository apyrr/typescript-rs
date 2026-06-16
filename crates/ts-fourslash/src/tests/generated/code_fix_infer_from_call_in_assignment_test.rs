#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_call_in_assignment() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_call_in_assignment(&mut t);
}

fn run_test_code_fix_infer_from_call_in_assignment(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixInferFromCallInAssignment") {
        return;
    }
    let content = r"// @noImplicitAny: true
function inferAny( [| app |] ) {
    const result = app.use('hi')
    return result
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "app: { use: (arg0: string) => any }", false, 0, 0);
    done();
}
