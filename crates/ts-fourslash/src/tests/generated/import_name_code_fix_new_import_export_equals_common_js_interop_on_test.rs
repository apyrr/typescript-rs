#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_export_equals_common_js_interop_on() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_export_equals_common_js_interop_on(&mut t);
}

fn run_test_import_name_code_fix_new_import_export_equals_common_js_interop_on(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportExportEqualsCommonJSInteropOn") {
        return;
    }
    let content = r#"// @Module: commonjs
// @EsModuleInterop: true
// @Filename: /foo.d.ts
declare module "bar" {
  const bar: number;
  export = bar;
}
declare module "foo" {
  const foo: number;
  export = foo;
}
declare module "es" {
  const es = 0;
  export default es;
}
// @Filename: /a.ts
import bar = require("bar");

foo
// @Filename: /b.ts
foo
// @Filename: /c.ts
import es from "es";
import bar = require("bar");

foo"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import bar = require("bar");
import foo = require("foo");

foo"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/b.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import foo from "foo";

foo"#
            .to_string()],
        None,
    );
    f.go_to_file(t, "/c.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import es from "es";
import bar = require("bar");
import foo = require("foo");

foo"#
            .to_string()],
        None,
    );
    done();
}
