#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_tsconfig_include_no_change() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_tsconfig_include_no_change(&mut t);
}

fn run_test_get_edits_for_file_rename_tsconfig_include_no_change(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /src/tsconfig.json
{
    "include": ["dir"],
}
// @Filename: /src/dir/a.ts
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/src/dir/a.ts",
        "/src/dir/b.ts",
        std::collections::HashMap::<String, String>::new(),
    );
    done();
}
