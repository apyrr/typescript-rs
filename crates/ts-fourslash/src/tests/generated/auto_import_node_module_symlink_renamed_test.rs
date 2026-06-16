#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_node_module_symlink_renamed() {
    let mut t = TestingT;
    run_test_auto_import_node_module_symlink_renamed(&mut t);
}

fn run_test_auto_import_node_module_symlink_renamed(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportNodeModuleSymlinkRenamed") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/solution/package.json
{
    "name": "monorepo",
    "workspaces": ["packages/*"]
}
// @Filename: /home/src/workspaces/solution/packages/utils/package.json
{
    "name": "utils",
    "version": "1.0.0",
    "exports": "./dist/index.js"
}
// @Filename: /home/src/workspaces/solution/packages/utils/tsconfig.json
{
    "compilerOptions": {
        "lib": ["es5"],
        "composite": true,
        "module": "nodenext",
        "rootDir": "src",
        "outDir": "dist"
    },
    "include": ["src"]
}
// @Filename: /home/src/workspaces/solution/packages/utils/src/index.ts
export function gainUtility() { return 0; }
// @Filename: /home/src/workspaces/solution/packages/web/package.json
{
    "name": "web",
    "version": "1.0.0",
    "dependencies": {
        "@monorepo/utils": "file:../utils"
    }
}
// @Filename: /home/src/workspaces/solution/packages/web/tsconfig.json
{
    "compilerOptions": {
        "lib": ["es5"],
        "composite": true,
        "module": "esnext",
        "moduleResolution": "bundler",
        "rootDir": "src",
        "outDir": "dist",
        "emitDeclarationOnly": true
    },
    "include": ["src"],
    "references": [
        { "path": "../utils" }
    ]
}
// @Filename: /home/src/workspaces/solution/packages/web/src/index.ts
gainUtility/**/
// @link: /home/src/workspaces/solution/packages/utils -> /home/src/workspaces/solution/node_modules/utils
// @link: /home/src/workspaces/solution/packages/utils -> /home/src/workspaces/solution/node_modules/@monorepo/utils
// @link: /home/src/workspaces/solution/packages/web -> /home/src/workspaces/solution/node_modules/web"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_module_specifiers(t, "", &vec!["@monorepo/utils".to_string()], None);
    done();
}
