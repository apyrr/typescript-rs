#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_incremental_server() {
    let mut t = TestingT;
    run_test_tsx_incremental_server(&mut t);
}

fn run_test_tsx_incremental_server(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.insert(t, "<");
    f.insert(t, "div");
    f.insert(t, " ");
    f.insert(t, " id");
    f.insert(t, "=");
    f.insert(t, "\"foo");
    f.insert(t, "\"");
    f.insert(t, ">");
    done();
}
