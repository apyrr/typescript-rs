#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_symlink() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_symlink(&mut t);
}

fn run_test_get_edits_for_file_rename_symlink(t: &mut TestingT) {
    if should_skip_if_failing("TestGetEditsForFileRename_symlink") {
        return;
    }
    let content = r"// @Filename: /foo.ts
// @Symlink: /node_modules/foo/index.ts
export const x = 0;
// @Filename: /user.ts
import { x } from 'foo';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_will_rename_files_edits(
        t,
        "/user.ts",
        "/luser.ts",
        std::collections::HashMap::<String, String>::new(),
    );
    done();
}
