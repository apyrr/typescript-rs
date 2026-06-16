#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_exports03() {
    let mut t = TestingT;
    run_test_rename_js_exports03(&mut t);
}

fn run_test_rename_js_exports03(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @Filename: a.js
class /*1*/A {
    /*2*/constructor() { }
}
module.exports = A;
// @Filename: b.js
const /*3*/A = require("./a");
new /*4*/A;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
