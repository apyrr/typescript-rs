#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_triple_slash_reference_resolution_mode() {
    let mut t = TestingT;
    run_test_triple_slash_reference_resolution_mode(&mut t);
}

fn run_test_triple_slash_reference_resolution_mode(t: &mut TestingT) {
    if should_skip_if_failing("TestTripleSlashReferenceResolutionMode") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.json
 { "compilerOptions": { "lib": ["es5"], "module": "nodenext", "declaration": true, "strict": true, "outDir": "out" }, "files": ["./index.ts"] }
// @Filename: /home/src/workspaces/project/package.json
 { "private": true, "type": "commonjs" }
// @Filename: /home/src/workspaces/project/node_modules/pkg/package.json
{ "name": "pkg", "version": "0.0.1", "exports": { "require": "./require.cjs", "default": "./import.js" }, "type": "module" }
// @Filename: /home/src/workspaces/project/node_modules/pkg/require.d.cts
export {};
export interface PkgRequireInterface { member: any; }
declare global { const pkgRequireGlobal: PkgRequireInterface; }
// @Filename: /home/src/workspaces/project/node_modules/pkg/import.d.ts
export {};
export interface PkgImportInterface { field: any; }
declare global { const pkgImportGlobal: PkgImportInterface; }
// @Filename: /home/src/workspaces/project/index.ts
/// <reference types="pkg" resolution-mode="import" />
pkgImportGlobal;
export {};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/home/src/workspaces/project/index.ts");
    f.verify_number_of_errors_in_current_file(0);
    done();
}
