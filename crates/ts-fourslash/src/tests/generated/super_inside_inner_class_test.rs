#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_super_inside_inner_class() {
    let mut t = TestingT;
    run_test_super_inside_inner_class(&mut t);
}

fn run_test_super_inside_inner_class(t: &mut TestingT) {
    if should_skip_if_failing("TestSuperInsideInnerClass") {
        return;
    }
    let content = r"class Base {
	constructor(n: number) {
	}
}
class Derived extends Base {
	constructor() {
		class Nested {
			[super(/*1*/)] = 11111
		}
	}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["1".to_string()]);
    done();
}
