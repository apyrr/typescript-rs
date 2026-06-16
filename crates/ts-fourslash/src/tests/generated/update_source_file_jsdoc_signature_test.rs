#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_update_source_file_jsdoc_signature() {
    let mut t = TestingT;
    run_test_update_source_file_jsdoc_signature(&mut t);
}

fn run_test_update_source_file_jsdoc_signature(t: &mut TestingT) {
    if should_skip_if_failing("TestUpdateSourceFile_jsdocSignature") {
        return;
    }
    let content = r"/**
 * @callback Cb
 * @return {/**/}
 */
let x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "number");
    done();
}
