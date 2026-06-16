#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source14_unresolved_require_destructuring() {
    let mut t = TestingT;
    run_test_go_to_source14_unresolved_require_destructuring(&mut t);
}

fn run_test_go_to_source14_unresolved_require_destructuring(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToSource14_unresolvedRequireDestructuring") {
        return;
    }
    let content = r#"// @lib: es5
// @allowJs: true
// @Filename: /home/src/workspaces/project/index.js
const { blah/**/ } = require("unresolved");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(t, &["".to_string()]);
    done();
}
