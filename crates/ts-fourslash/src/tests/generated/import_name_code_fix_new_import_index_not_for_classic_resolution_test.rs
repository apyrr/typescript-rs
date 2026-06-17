#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_index_not_for_classic_resolution() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_index_not_for_classic_resolution(&mut t);
}

fn run_test_import_name_code_fix_new_import_index_not_for_classic_resolution(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportIndex_notForClassicResolution") {
        return;
    }
    let content = r"// @moduleResolution: classic
// @Filename: /a/index.ts
export const foo = 0;
// @Filename: /node_modules/x/index.d.ts
export const bar = 0;
// @Filename: /b.ts
[|foo;|]
// @Filename: /c.ts
[|bar;|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a/index.ts");
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { foo } from "./a/index";

foo;"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/c.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { bar } from "./node_modules/x/index";

bar;"#
            .to_string()],
        None,
    );
    done();
}
