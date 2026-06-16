#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_no_format_options() {
    let mut t = TestingT;
    run_test_organize_imports_no_format_options(&mut t);
}

fn run_test_organize_imports_no_format_options(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsNoFormatOptions") {
        return;
    }
    let content = r#"import {
  stat,
  statSync,
} from "fs";
export function fakeFn() {
  stat;
  statSync;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import {
stat,
statSync,
} from "fs";
export function fakeFn() {
  stat;
  statSync;
}"#,
        "source.organizeImports",
        None,
    );
    done();
}
