#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_ambient_module() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_ambient_module(&mut t);
}

fn run_test_get_edits_for_file_rename_ambient_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /tsconfig.json
{}
// @Filename: /sub/types.d.ts
// @Symlink: /node_modules/sub/types.d.ts
declare module "sub" {
    declare export const abc: number
}
// @Filename: /sub/package.json
// @Symlink: /node_modules/sub/package.json
{ "types": "types.d.ts" }
// @Filename: /a.ts
import { abc } from "sub";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/a.ts",
        "/b.ts",
        std::collections::HashMap::<String, String>::new(),
    );
    done();
}
