#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_enum_update1() {
    let mut t = TestingT;
    run_test_enum_update1(&mut t);
}

fn run_test_enum_update1(t: &mut TestingT) {
    if should_skip_if_failing("TestEnumUpdate1") {
        return;
    }
    let content = r"namespace M {
	export enum E {
		A = 1,
		B = 2,
		C = 3,
		/*1*/
	}
}
namespace M {
	function foo(): M.E {
		return M.E.A;
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.insert(t, "D = C << 1,");
    f.verify_no_errors();
    done();
}
