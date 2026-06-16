#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_umd_global_java_script() {
    let mut t = TestingT;
    run_test_import_name_code_fix_umd_global_java_script(&mut t);
}

fn run_test_import_name_code_fix_umd_global_java_script(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixUMDGlobalJavaScript") {
        return;
    }
    let content = r"// @AllowSyntheticDefaultImports: false
// @Module: commonjs
// @CheckJs: true
// @AllowJs: true
// @Filename: a/f1.js
[|export function test() { };
bar1/*0*/.bar;|]
// @Filename: a/foo.d.ts
export declare function bar(): number;
export as namespace bar1; ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import * as bar1 from "./foo";

export function test() { };
bar1.bar;"#
                .to_string(),
        ],
        None,
    );
    done();
}
