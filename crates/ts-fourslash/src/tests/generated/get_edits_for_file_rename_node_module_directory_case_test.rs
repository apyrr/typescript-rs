#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_node_module_directory_case() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_node_module_directory_case(&mut t);
}

fn run_test_get_edits_for_file_rename_node_module_directory_case(t: &mut TestingT) {
    if should_skip_if_failing("TestGetEditsForFileRename_nodeModuleDirectoryCase") {
        return;
    }
    let content = r#"// @Filename: /a/b/file1.ts
import { foo } from "foo";
// @Filename: /a/node_modules/foo/index.d.ts
export const foo = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/a/b",
        "/a/B",
        std::collections::HashMap::<String, String>::new(),
    );
    done();
}
