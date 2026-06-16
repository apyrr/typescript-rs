#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tslib_find_all_references_on_runtime_import_with_paths1() {
    let mut t = TestingT;
    run_test_tslib_find_all_references_on_runtime_import_with_paths1(&mut t);
}

fn run_test_tslib_find_all_references_on_runtime_import_with_paths1(t: &mut TestingT) {
    if should_skip_if_failing("TestTslibFindAllReferencesOnRuntimeImportWithPaths1") {
        return;
    }
    let content = r#"// @Filename: project/src/foo.ts
import * as x from /**/"tslib";
// @Filename: project/src/bar.ts
export default "";
// @Filename: project/src/bal.ts

// @Filename: project/src/dir/tslib.d.ts
export function __importDefault(...args: any): any;
export function __importStar(...args: any): any;
// @Filename: project/tsconfig.json
{
    "compilerOptions": {
        "moduleResolution": "node",
        "module": "es2020",
        "importHelpers": true,
        "moduleDetection": "force",
        "paths": {
            "tslib": ["./src/dir/tslib"]
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
