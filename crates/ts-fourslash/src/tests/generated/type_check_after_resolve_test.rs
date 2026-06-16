#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_check_after_resolve() {
    let mut t = TestingT;
    run_test_type_check_after_resolve(&mut t);
}

fn run_test_type_check_after_resolve(t: &mut TestingT) {
    if should_skip_if_failing("TestTypeCheckAfterResolve") {
        return;
    }
    let content = r"/*start*/class Point implements /*IPointRef*/IPoint {
    getDist() {
        ssss;
    }
}/*end*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
    f.insert_line(t, "");
    f.verify_quick_info_at(t, "IPointRef", "any", "");
    f.verify_error_exists_after_marker_name("IPointRef");
    f.go_to_eof(t);
    f.insert_line(t, "");
    f.verify_error_exists_after_marker_name("IPointRef");
    done();
}
