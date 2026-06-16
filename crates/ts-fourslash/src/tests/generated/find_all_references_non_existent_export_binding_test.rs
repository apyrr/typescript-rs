#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_non_existent_export_binding() {
    let mut t = TestingT;
    run_test_find_all_references_non_existent_export_binding(&mut t);
}

fn run_test_find_all_references_non_existent_export_binding(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesNonExistentExportBinding") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
 { "compilerOptions": { "module": "commonjs" } }
// @filename: /bar.ts
import { Foo/**/ } from "./foo";
// @filename: /foo.ts
export { Foo }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
