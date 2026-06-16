#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rewrite_relative_import_extensions_project_references3() {
    let mut t = TestingT;
    run_test_rewrite_relative_import_extensions_project_references3(&mut t);
}

fn run_test_rewrite_relative_import_extensions_project_references3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: src/tsconfig-base.json
{
    "compilerOptions": {
        "lib": ["es5"],
        "module": "nodenext",
        "composite": true,
        "rewriteRelativeImportExtensions": true,
    }
}
// @Filename: src/compiler/tsconfig.json
{
    "extends": "../tsconfig-base.json",
    "compilerOptions": {
        "lib": ["es5"],
        "rootDir": ".",
        "outDir": "../../dist/compiler",
}
// @Filename: src/compiler/parser.ts
export {};
// @Filename: src/services/tsconfig.json
{
    "extends": "../tsconfig-base.json",
    "compilerOptions": {
        "lib": ["es5"],
        "rootDir": ".",
        "outDir": "../../dist/services",
    },
    "references": [
        { "path": "../compiler" }
    ]
}
// @Filename: src/services/services.ts
import {} from "../compiler/parser.ts";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/src/services/services.ts");
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}
