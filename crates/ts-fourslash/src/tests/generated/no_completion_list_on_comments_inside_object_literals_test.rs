#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_no_completion_list_on_comments_inside_object_literals() {
    let mut t = TestingT;
    run_test_no_completion_list_on_comments_inside_object_literals(&mut t);
}

fn run_test_no_completion_list_on_comments_inside_object_literals(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace ObjectLiterals {
	interface MyPoint {
		x1: number;
		y1: number;
	}

	var p1: MyPoint = {
		/* /*1*/ Comment /*2*/ */
	};
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Markers(f.markers()), None);
    done();
}
