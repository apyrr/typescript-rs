#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_node_next_auto_import3() {
    let mut t = TestingT;
    run_test_module_node_next_auto_import3(&mut t);
}

fn run_test_module_node_next_auto_import3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /tsconfig.json
{ "compilerOptions": { "module": "nodenext" } }
// @Filename: /package.json
{ "type": "module" }
// @Filename: /mobx.d.mts
export declare function autorun(): void;
// @Filename: /index.ts
autorun/**/
// @Filename: /utils.ts
import "./mobx.mjs";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { autorun } from "./mobx.mjs";

autorun"#
                .to_string(),
        ],
        None,
    );
    done();
}
