#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_import_type1() {
    let mut t = TestingT;
    run_test_inlay_hints_import_type1(&mut t);
}

fn run_test_inlay_hints_import_type1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: /a.js
module.exports.a = 1
// @Filename: /b.js
const a = require('./a');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.js");
    f.verify_baseline_inlay_hints(t);
    done();
}
