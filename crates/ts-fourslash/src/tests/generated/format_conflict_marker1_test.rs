#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_conflict_marker1() {
    let mut t = TestingT;
    run_test_format_conflict_marker1(&mut t);
}

fn run_test_format_conflict_marker1(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatConflictMarker1") {
        return;
    }
    let content = r"class C {
<<<<<<< HEAD
v = 1;
=======
v = 2;
>>>>>>> Branch - a
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"class C {
<<<<<<< HEAD
    v = 1;
=======
v = 2;
>>>>>>> Branch - a
}",
    );
    done();
}
