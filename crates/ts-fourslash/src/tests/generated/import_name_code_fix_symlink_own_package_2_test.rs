#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_symlink_own_package_2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_symlink_own_package_2(&mut t);
}

fn run_test_import_name_code_fix_symlink_own_package_2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_symlink_own_package_2") {
        return;
    }
    let content = r#"// @Filename: /packages/a/test.ts
// @Symlink: /node_modules/a/test.ts
x;
// @Filename: /packages/a/utils.ts
// @Symlink: /node_modules/a/utils.ts
import {} from "a/utils";
export const x = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/packages/a/test.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { x } from "./utils";

x;"#
            .to_string(),
        ],
        None,
    );
    done();
}
