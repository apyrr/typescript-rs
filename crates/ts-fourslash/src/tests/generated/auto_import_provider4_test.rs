#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider4() {
    let mut t = TestingT;
    run_test_auto_import_provider4(&mut t);
}

fn run_test_auto_import_provider4(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportProvider4") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/a/package.json
{ "dependencies": { "b": "*" } }
// @Filename: /home/src/workspaces/project/a/tsconfig.json
{ "compilerOptions": { "lib": ["es5"], "module": "commonjs", "target": "esnext" }, "references": [{ "path": "../b" }] }
// @Filename: /home/src/workspaces/project/a/index.ts
new Shape/**/
// @Filename: /home/src/workspaces/project/b/package.json
{ "types": "out/index.d.ts" }
// @Filename: /home/src/workspaces/project/b/tsconfig.json
{ "compilerOptions": { "lib": ["es5"], "outDir": "out", "composite": true } }
// @Filename: /home/src/workspaces/project/b/index.ts
export class Shape {}
// @link: /home/src/workspaces/project/b -> /home/src/workspaces/project/a/node_modules/b"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Shape } from "b";

new Shape"#
                .to_string(),
        ],
        None,
    );
    done();
}
