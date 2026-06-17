#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_pnpm() {
    let mut t = TestingT;
    run_test_auto_import_pnpm(&mut t);
}

fn run_test_auto_import_pnpm(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPnpm") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{ "compilerOptions": { "module": "commonjs" } }
// @Filename: /node_modules/.pnpm/mobx@6.0.4/node_modules/mobx/package.json
{ "types": "dist/mobx.d.ts" }
// @Filename: /node_modules/.pnpm/mobx@6.0.4/node_modules/mobx/dist/mobx.d.ts
export declare function autorun(): void;
// @Filename: /index.ts
autorun/**/
// @Filename: /utils.ts
import "mobx";
// @link: /node_modules/.pnpm/mobx@6.0.4/node_modules/mobx -> /node_modules/mobx
// @link: /node_modules/.pnpm/mobx@6.0.4/node_modules/mobx -> /node_modules/.pnpm/cool-mobx-dependent@1.2.3/node_modules/mobx"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
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
