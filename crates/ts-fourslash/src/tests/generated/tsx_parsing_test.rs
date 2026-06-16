#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_parsing() {
    let mut t = TestingT;
    run_test_tsx_parsing(&mut t);
}

fn run_test_tsx_parsing(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxParsing") {
        return;
    }
    let content = r#"var x = <div id="foo" master="bar"></div>;
var y = /**/x;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
