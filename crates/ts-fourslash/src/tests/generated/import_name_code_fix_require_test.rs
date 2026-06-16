#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_require() {
    let mut t = TestingT;
    run_test_import_name_code_fix_require(&mut t);
}

fn run_test_import_name_code_fix_require(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_require") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: foo.js
module.exports = function foo() {}
// @Filename: utils.js
function util1() {}
function util2() {}
module.exports = { util1, util2 };
// @Filename: blah.js
export default class Blah {}
// @Filename: index.js
foo();
util1();
util2();
new Blah;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.js");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"const { default: Blah } = require("./blah");
const foo = require("./foo");
const { util1, util2 } = require("./utils");

foo();
util1();
util2();
new Blah;"#
                .to_string(),
        },
    );
    done();
}
