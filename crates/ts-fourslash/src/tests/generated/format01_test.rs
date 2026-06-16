#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format01() {
    let mut t = TestingT;
    run_test_format01(&mut t);
}

fn run_test_format01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
/**/namespace Default{var x= ( { } ) ;}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.format_document(t, "");
    f.go_to_marker(t, "");
    f.verify_current_line_content(t, "namespace Default { var x = ({}); }");
    done();
}
