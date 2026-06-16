#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_import_type2() {
    let mut t = TestingT;
    run_test_inlay_hints_import_type2(&mut t);
}

fn run_test_inlay_hints_import_type2(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsImportType2") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: /a.js
module.exports.a = 1
// @Filename: /b.js
function foo () { return require('./a'); }
function bar () { return require('./a').a; }
const c = foo()
const d = bar()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.js");
    f.verify_baseline_inlay_hints(t);
    done();
}
