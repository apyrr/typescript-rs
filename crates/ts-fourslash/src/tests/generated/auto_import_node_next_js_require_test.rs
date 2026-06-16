#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_node_next_js_require() {
    let mut t = TestingT;
    run_test_auto_import_node_next_js_require(&mut t);
}

fn run_test_auto_import_node_next_js_require(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: node18
// @allowJs: true
// @checkJs: true
// @noEmit: true
// @Filename: /matrix.js
exports.variants = [];
// @Filename: /main.js
exports.dedupeLines = data => {
  variants/**/
}
// @Filename: /totally-irrelevant-no-way-this-changes-things-right.js
export default 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/main.js");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"const { variants } = require("./matrix")

exports.dedupeLines = data => {
  variants
}"#
            .to_string(),
        ],
        None,
    );
    done();
}
