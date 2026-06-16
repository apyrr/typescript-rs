#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_destructured_require2() {
    let mut t = TestingT;
    run_test_go_to_definition_destructured_require2(&mut t);
}

fn run_test_go_to_definition_destructured_require2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @Filename: util.js
class /*2*/Util {}
module.exports = { Util };
// @Filename: reexport.js
const { Util } = require('./util');
module.exports = { Util };
// @Filename: index.js
const { Util } = require('./reexport');
new [|Util/*1*/|]()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
