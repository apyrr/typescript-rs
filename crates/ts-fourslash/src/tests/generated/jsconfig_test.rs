#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsconfig() {
    let mut t = TestingT;
    run_test_jsconfig(&mut t);
}

fn run_test_jsconfig(t: &mut TestingT) {
    if should_skip_if_failing("TestJsconfig") {
        return;
    }
    let content = r#"// @Filename: /a.js
function f(/**/x) {
}
// @Filename: /jsconfig.json
{
    "compilerOptions": {
        "checkJs": true,
        "noImplicitAny": true
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.js");
    f.verify_error_exists_after_marker_name("");
    done();
}
