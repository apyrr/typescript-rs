#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_function_this_usage_object_property() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_function_this_usage_object_property(&mut t);
}

fn run_test_code_fix_infer_from_function_this_usage_object_property(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixInferFromFunctionThisUsageObjectProperty") {
        return;
    }
    let content = r#"// @noImplicitThis: true
function returnThisMember([| |]) {
     return this.member;
 }

 interface Container {
     member: string;
     returnThisMember(): string;
 }

 const container: Container = {
     member: "sample",
     returnThisMember: returnThisMember,
 };

 container.returnThisMember();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "this: Container", false, 0, 0);
    done();
}
