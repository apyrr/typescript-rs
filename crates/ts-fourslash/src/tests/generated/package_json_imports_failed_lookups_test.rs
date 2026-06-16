#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_package_json_imports_failed_lookups() {
    let mut t = TestingT;
    run_test_package_json_imports_failed_lookups(&mut t);
}

fn run_test_package_json_imports_failed_lookups(t: &mut TestingT) {
    if should_skip_if_failing("TestPackageJsonImportsFailedLookups") {
        return;
    }
    let content = r##"// @Filename: /a/b/c/d/e/tsconfig.json
{ "compilerOptions": { "lib": ["es5"], "module": "nodenext" } }
// @Filename: /a/b/c/d/e/package.json
{
  "name": "app",
  "imports": {
    "#utils": "lodash"
  }
}
// @Filename: /a/b/node_modules/lodash/index.d.ts
export function add(a: number, b: number): number;
// @Filename: /a/b/c/d/e/index.ts
import { add } from "#utils";"##;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/a/b/c/d/e/index.ts");
    done();
}
