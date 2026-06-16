#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_named() {
    let mut t = TestingT;
    run_test_find_all_refs_import_named(&mut t);
}

fn run_test_find_all_refs_import_named(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsImportNamed") {
        return;
    }
    let content = r#"// @module: commonjs
// @Filename: f.ts
export { foo as foo }
function /*start*/foo(a: number, b: number) { }
// @Filename: b.ts
import x = require("./f");
x.foo(1, 2);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["start".to_string()]);
    done();
}
