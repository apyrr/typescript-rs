#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_with_keyword() {
    let mut t = TestingT;
    run_test_completions_import_with_keyword(&mut t);
}

fn run_test_completions_import_with_keyword(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsImportWithKeyword") {
        return;
    }
    let content = r#"// @lib: es5
// @allowJs: true
// @Filename: a.ts
 const f = {
    a: 1
};
 import * as thing from "thing" /*0*/
 export { foo } from "foo" /*1*/
 import "foo" as /*2*/
 import "foo" w/*3*/
 import * as that from "that"
 /*4*/
 import * /*5*/ as those from "those"
// @Filename: b.js
 import * as thing from "thing" /*js*/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
