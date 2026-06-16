#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_rename03() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_rename03(&mut t);
}

fn run_test_jsdoc_typedef_tag_rename03(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagRename03") {
        return;
    }
    let content = r#"// @lib: es5
// @allowNonTsExtensions: true
// @Filename: jsDocTypedef_form3.js

/**
 * [|@typedef /*1*/[|{| "contextRangeIndex": 0 |}Person|]
 * @type {Object}
 * @property {number} age
 * @property {string} name
 |]*/

/** @type {/*2*/[|Person|]} */
var person;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "jsDocTypedef_form3.js");
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.get_ranges_by_text("Person")
            .into_iter()
            .map(Into::into)
            .collect(),
    );
    done();
}
