#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_node_next_auto_import1() {
    let mut t = TestingT;
    run_test_module_node_next_auto_import1(&mut t);
}

fn run_test_module_node_next_auto_import1(t: &mut TestingT) {
    if should_skip_if_failing("TestModuleNodeNextAutoImport1") {
        return;
    }
    let content = r#"// @Filename: /tsconfig.json
{ "compilerOptions": { "module": "nodenext" } }
// @Filename: /package.json
{ "type": "module" }
// @Filename: /mobx.d.ts
export declare function autorun(): void;
// @Filename: /index.ts
autorun/**/
// @Filename: /utils.ts
import "./mobx.js";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { autorun } from "./mobx.js";

autorun"#
                .to_string(),
        ],
        None,
    );
    done();
}
