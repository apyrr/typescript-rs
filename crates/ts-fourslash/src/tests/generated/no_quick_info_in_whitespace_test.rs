#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_no_quick_info_in_whitespace() {
    let mut t = TestingT;
    run_test_no_quick_info_in_whitespace(&mut t);
}

fn run_test_no_quick_info_in_whitespace(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
/*1*/    private _mspointerupHandler(args) {
        if (args.button === 3) {
            return null; 
/*2*/        } else if (args.button === 4) {
/*3*/            return null;
        }
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_not_quick_info_exists(t);
    f.go_to_marker(t, "2");
    f.verify_not_quick_info_exists(t);
    f.go_to_marker(t, "3");
    f.verify_not_quick_info_exists(t);
    done();
}
