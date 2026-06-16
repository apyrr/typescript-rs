#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indirect_js_require_rename() {
    let mut t = TestingT;
    run_test_indirect_js_require_rename(&mut t);
}

fn run_test_indirect_js_require_rename(t: &mut TestingT) {
    if should_skip_if_failing("TestIndirectJsRequireRename") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /bin/serverless.js
require('../lib/classes/Error').log/**/Warning(`CLI triage crashed with: ${error.stack}`);
// @Filename: /lib/plugins/aws/package/compile/events/httpApi/index.js
const { logWarning } = require('../../../../../../classes/Error');
// @Filename: /lib/classes/Error.js
module.exports.logWarning = message => { };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
