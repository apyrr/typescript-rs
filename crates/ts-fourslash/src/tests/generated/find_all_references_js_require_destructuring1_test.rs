#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_js_require_destructuring1() {
    let mut t = TestingT;
    run_test_find_all_references_js_require_destructuring1(&mut t);
}

fn run_test_find_all_references_js_require_destructuring1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @noEmit: true
// @checkJs: true
// @Filename: /X.js
module.exports = { x: 1 };
// @Filename: /Y.js
const { /*1*/x: { y } } = require("./X");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
