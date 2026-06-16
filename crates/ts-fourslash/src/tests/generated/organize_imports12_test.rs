#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports12() {
    let mut t = TestingT;
    run_test_organize_imports12(&mut t);
}

fn run_test_organize_imports12(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports12") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: /test.js
declare export default class A {}
declare export { a, b };
declare export * from "foo";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"declare export default class A {}
declare export * from "foo";
declare export { a, b };
"#,
        "source.organizeImports",
        None,
    );
    done();
}
