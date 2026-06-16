#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_export() {
    let mut t = TestingT;
    run_test_quick_info_js_export(&mut t);
}

fn run_test_quick_info_js_export(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJSExport") {
        return;
    }
    let content = r#"// @Filename: a.js
// @allowJs: true
/**
 * @enum {string}
 */
const testString = {
    one: "1",
    two: "2"
};

export { test/**/String };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(alias) type testString = string\n(alias) const testString: {\n    one: string;\n    two: string;\n}\nexport testString", "");
    done();
}
