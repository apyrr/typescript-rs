#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rewrite_relative_import_extensions_project_references1() {
    let mut t = TestingT;
    run_test_rewrite_relative_import_extensions_project_references1(&mut t);
}

fn run_test_rewrite_relative_import_extensions_project_references1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: packages/common/tsconfig.json
{
    "compilerOptions": {
        "lib": ["es5"],
        "composite": true,
        "rootDir": "src",
        "outDir": "dist",
        "module": "nodenext",
        "resolveJsonModule": false,
    }
}
// @Filename: packages/common/package.json
{
    "name": "common",
    "version": "1.0.0",
    "type": "module",
    "exports": {
        ".": {
            "source": "./src/index.ts",
            "default": "./dist/index.js"
        }
    }
}
// @Filename: packages/common/src/index.ts
export {};
// @Filename: packages/main/tsconfig.json
{
    "compilerOptions": {
        "module": "nodenext",
        "rewriteRelativeImportExtensions": true,
        "lib": ["es5"],
        "rootDir": "src",
        "outDir": "dist",
        "resolveJsonModule": false,
    },
    "references": [
        { "path": "../common" }
    ]
}
// @Filename: packages/main/package.json
{ "type": "module" }
// @Filename: packages/main/src/index.ts
import {} from "../../common/src/index.ts";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/packages/main/src/index.ts");
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}
