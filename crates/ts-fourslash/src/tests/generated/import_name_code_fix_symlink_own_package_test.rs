#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_symlink_own_package() {
    let mut t = TestingT;
    run_test_import_name_code_fix_symlink_own_package(&mut t);
}

fn run_test_import_name_code_fix_symlink_own_package(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_symlink_own_package") {
        return;
    }
    let content = r#"// @Filename: /packages/b/b0.ts
// @Symlink: /node_modules/b/b0.ts
x;
// @Filename: /packages/b/b1.ts
// @Symlink: /node_modules/b/b1.ts
import { a } from "a";
export const x = 0;
// @Filename: /packages/a/index.d.ts
// @Symlink: /node_modules/a/index.d.ts
export const a: number;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/packages/b/b0.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { x } from "./b1";

x;"#
            .to_string(),
        ],
        None,
    );
    done();
}
