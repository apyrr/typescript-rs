#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_hover_over_comment() {
    let mut t = TestingT;
    run_test_hover_over_comment(&mut t);
}

fn run_test_hover_over_comment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export function f() {}
//foo
/**///moo";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "", "");
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    f.verify_baseline_go_to_definition(t, &["".to_string()]);
    done();
}
