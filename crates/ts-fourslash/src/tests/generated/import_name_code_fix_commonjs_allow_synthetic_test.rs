#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_commonjs_allow_synthetic() {
    let mut t = TestingT;
    run_test_import_name_code_fix_commonjs_allow_synthetic(&mut t);
}

fn run_test_import_name_code_fix_commonjs_allow_synthetic(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: esnext
// @moduleResolution: bundler
// @allowJs: true
// @checkJs: true
// @allowSyntheticDefaultImports: true
// @Filename: /test_module.js
const MY_EXPORTS = {}
module.exports = MY_EXPORTS;
// @Filename: /index.js
const newVar = {
  any: MY_EXPORTS/**/,
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"const MY_EXPORTS = require("./test_module");

const newVar = {
  any: MY_EXPORTS,
}"#
            .to_string(),
        ],
        None,
    );
    done();
}
