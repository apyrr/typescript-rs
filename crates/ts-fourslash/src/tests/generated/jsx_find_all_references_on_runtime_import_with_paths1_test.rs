#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_find_all_references_on_runtime_import_with_paths1() {
    let mut t = TestingT;
    run_test_jsx_find_all_references_on_runtime_import_with_paths1(&mut t);
}

fn run_test_jsx_find_all_references_on_runtime_import_with_paths1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: project/src/foo.ts
import * as x from /**/"@foo/dir/jsx-runtime";
// @Filename: project/src/bar.tsx
export default <div></div>;
// @Filename: project/src/baz.tsx
export default <></>;
// @Filename: project/src/bam.tsx
export default <script src=""/>;
// @Filename: project/src/bat.tsx
export const a = 1;
// @Filename: project/src/bal.tsx

// @Filename: project/src/dir/jsx-runtime.ts
export {}
// @Filename: project/tsconfig.json
{
    "compilerOptions": {
        "moduleResolution": "node",
        "module": "es2020",
        "jsx": "react-jsx",
        "jsxImportSource": "@foo/dir",
        "moduleDetection": "force",
        "paths": {
            "@foo/dir/jsx-runtime": ["./src/dir/jsx-runtime"]
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
