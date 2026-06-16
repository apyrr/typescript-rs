#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_localsin_constructor_fs2() {
    let mut t = TestingT;
    run_test_unused_localsin_constructor_fs2(&mut t);
}

fn run_test_unused_localsin_constructor_fs2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noUnusedLocals: true
// @noUnusedParameters: true
class greeter {
    [|constructor() {
        var unused = 20;
        var used = "dummy";
        used = used + "second part";
    }|]
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\n    constructor() {\n        var used = \"dummy\";\n        used = used + \"second part\";\n    }\n", false, 0, 0);
    done();
}
