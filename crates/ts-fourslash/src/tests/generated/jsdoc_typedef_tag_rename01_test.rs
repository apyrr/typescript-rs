#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_rename01() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_rename01(&mut t);
}

fn run_test_jsdoc_typedef_tag_rename01(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagRename01") {
        return;
    }
    let content = r#"// @lib: es5
// @allowNonTsExtensions: true
// @Filename: jsDocTypedef_form1.js

/** @typedef {(string | number)} */
[|var [|{| "contextRangeIndex": 0 |}NumberLike|];|]

[|NumberLike|] = 10;

/** @type {[|NumberLike|]} */
var numberLike;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.ranges()[1..].iter().cloned().map(Into::into).collect(),
    );
    done();
}
