#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_rename04() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_rename04(&mut t);
}

fn run_test_jsdoc_typedef_tag_rename04(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagRename04") {
        return;
    }
    let content = r"// @lib: es5
// @allowNonTsExtensions: true
// @Filename: jsDocTypedef_form2.js

function test1() {
   /** @typedef {(string | number)} NumberLike */

   /** @type {/*1*/NumberLike} */
   var numberLike;
}
function test2() {
   /** @typedef {(string | number)} NumberLike2 */

   /** @type {NumberLike2} */
   var n/*2*/umberLike2;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "2");
    f.verify_quick_info_exists(t);
    f.go_to_marker(t, "1");
    f.insert(t, "111");
    f.go_to_marker(t, "2");
    f.verify_quick_info_exists(t);
    done();
}
