#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_type_of_this_in_statics() {
    let mut t = TestingT;
    run_test_quick_info_type_of_this_in_statics(&mut t);
}

fn run_test_quick_info_type_of_this_in_statics(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    static foo() {
        var /*1*/r = this;
    }
    static get x() {
        var /*2*/r = this;
        return 1;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(local var) r: typeof C", "");
    f.verify_quick_info_at(t, "2", "(local var) r: typeof C", "");
    done();
}
