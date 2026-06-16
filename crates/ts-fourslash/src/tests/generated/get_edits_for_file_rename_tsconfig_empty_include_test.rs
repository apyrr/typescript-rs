#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_tsconfig_empty_include() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_tsconfig_empty_include(&mut t);
}

fn run_test_get_edits_for_file_rename_tsconfig_empty_include(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a/foo.ts
const x = 1
// @Filename: /a/tsconfig.json
{ "include": [] }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/a/foo.ts",
        "/a/bar.ts",
        std::collections::HashMap::<String, String>::new(),
    );
    done();
}
