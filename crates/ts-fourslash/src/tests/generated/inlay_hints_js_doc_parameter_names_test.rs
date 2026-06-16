#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_js_doc_parameter_names() {
    let mut t = TestingT;
    run_test_inlay_hints_js_doc_parameter_names(&mut t);
}

fn run_test_inlay_hints_js_doc_parameter_names(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsJsDocParameterNames") {
        return;
    }
    let content = r#"// @allowJs: true
// @checkJs: true
// @Filename: /a.js
var x
x.foo(1, 2);
/**
 * @type {{foo: (a: number, b: number) => void}}
 */
var y
y.foo(1, 2)
/**
 * @type {string}
 */
var z = """#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.js");
    f.verify_baseline_inlay_hints(t);
    done();
}
