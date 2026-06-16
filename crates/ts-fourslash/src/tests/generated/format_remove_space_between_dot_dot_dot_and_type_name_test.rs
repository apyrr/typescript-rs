#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_remove_space_between_dot_dot_dot_and_type_name() {
    let mut t = TestingT;
    run_test_format_remove_space_between_dot_dot_dot_and_type_name(&mut t);
}

fn run_test_format_remove_space_between_dot_dot_dot_and_type_name(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatRemoveSpaceBetweenDotDotDotAndTypeName") {
        return;
    }
    let content = r"let a: [... any[]];
let b: [...   number[]];
let c: [...     string[]];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"let a: [...any[]];
let b: [...number[]];
let c: [...string[]];",
    );
    done();
}
