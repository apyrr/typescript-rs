#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_json_imports_pattern_ts_js() {
    let mut t = TestingT;
    run_test_auto_import_package_json_imports_pattern_ts_js(&mut t);
}

fn run_test_auto_import_package_json_imports_pattern_ts_js(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r##"// @module: node18
// @Filename: /package.json
{
  "imports": {
    "#*.ts": "./src/*.js"
  }
}
// @Filename: /src/something.ts
export function something(name: string): any;
// @Filename: /a.ts
something/**/"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["#something.ts".to_string()], None);
    done();
}
