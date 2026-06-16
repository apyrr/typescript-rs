#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_export_equals_es_next_interop_on() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_export_equals_es_next_interop_on(&mut t);
}

fn run_test_import_name_code_fix_new_import_export_equals_es_next_interop_on(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportExportEqualsESNextInteropOn") {
        return;
    }
    let content = r#"// @EsModuleInterop: true
// @Module: es2015
// @Filename: /foo.d.ts
declare module "foo" {
  const foo: number;
  export = foo;
}
// @Filename: /index.ts
[|foo|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/index.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import foo from "foo";

foo"#
                .to_string(),
        ],
        None,
    );
    done();
}
