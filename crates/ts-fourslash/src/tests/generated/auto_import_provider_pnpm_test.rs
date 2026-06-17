#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider_pnpm() {
    let mut t = TestingT;
    run_test_auto_import_provider_pnpm(&mut t);
}

fn run_test_auto_import_provider_pnpm(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportProvider_pnpm") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.json
{ "compilerOptions": { "module": "commonjs", "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/package.json
{ "dependencies": { "mobx": "*" } }
// @Filename: /home/src/workspaces/project/node_modules/.pnpm/mobx@6.0.4/node_modules/mobx/package.json
{ "types": "dist/mobx.d.ts" }
// @Filename: /home/src/workspaces/project/node_modules/.pnpm/mobx@6.0.4/node_modules/mobx/dist/mobx.d.ts
export declare function autorun(): void;
// @Filename: /home/src/workspaces/project/index.ts
autorun/**/
// @link: /home/src/workspaces/project/node_modules/.pnpm/mobx@6.0.4/node_modules/mobx -> /home/src/workspaces/project/node_modules/mobx"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { autorun } from "mobx";

autorun"#
            .to_string()],
        None,
    );
    done();
}
