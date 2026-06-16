#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_allow_synthetic_default_imports0() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_allow_synthetic_default_imports0(&mut t);
}

fn run_test_import_name_code_fix_new_import_allow_synthetic_default_imports0(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportAllowSyntheticDefaultImports0") {
        return;
    }
    let content = r"// @AllowSyntheticDefaultImports: true
// @Filename: a/f1.ts
[|export var x = 0;
bar/*0*/();|]
// @Filename: a/foo.d.ts
declare function bar(): number;
export = bar;
export as namespace bar;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import bar from "./foo";

export var x = 0;
bar();"#
                .to_string(),
        ],
        None,
    );
    done();
}
