#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_fix_with_multiple_module_export_assignment() {
    let mut t = TestingT;
    run_test_import_fix_with_multiple_module_export_assignment(&mut t);
}

fn run_test_import_fix_with_multiple_module_export_assignment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: esnext
// @allowJs: true
// @checkJs: true
// @Filename: /a.js
function f() {}
module.exports = f;
module.exports = 42;
// @Filename: /b.js
export const foo = 0;
// @Filename: /c.js
foo";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/c.js");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"const { foo } = require("./b");

foo"#
                .to_string(),
        ],
        None,
    );
    done();
}
