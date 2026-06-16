#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_fixes_global_typings_cache() {
    let mut t = TestingT;
    run_test_import_fixes_global_typings_cache(&mut t);
}

fn run_test_import_fixes_global_typings_cache(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /project/tsconfig.json
 { "compilerOptions": { "allowJs": true, "checkJs": true, "module": "commonjs" } }
// @Filename: /home/src/Library/Caches/typescript/node_modules/@types/react-router-dom/package.json
 { "name": "@types/react-router-dom", "version": "16.8.4", "types": "index.d.ts" }
// @Filename: /home/src/Library/Caches/typescript/node_modules/@types/react-router-dom/index.d.ts
export class BrowserRouter {}
// @Filename: /project/node_modules/react-router-dom/package.json
 { "name": "react-router-dom", "version": "16.8.4", "main": "index.js" }
// @Filename: /project/node_modules/react-router-dom/index.js
 export const BrowserRouter = () => null;
// @Filename: /project/index.js
BrowserRouter/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/project/index.js");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"const { BrowserRouter } = require("react-router-dom");

BrowserRouter"#
                .to_string(),
        ],
        None,
    );
    done();
}
