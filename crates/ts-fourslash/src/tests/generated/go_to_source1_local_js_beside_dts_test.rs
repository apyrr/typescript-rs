#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source1_local_js_beside_dts() {
    let mut t = TestingT;
    run_test_go_to_source1_local_js_beside_dts(&mut t);
}

fn run_test_go_to_source1_local_js_beside_dts(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToSource1_localJsBesideDts") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: /home/src/workspaces/project/a.js
export const /*end*/a = "a";
// @Filename: /home/src/workspaces/project/a.d.ts
export declare const a: string;
// @Filename: /home/src/workspaces/project/index.ts
import { a } from [|"./a"/*moduleSpecifier*/|];
[|a/*identifier*/|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(
        t,
        &["identifier".to_string(), "moduleSpecifier".to_string()],
    );
    done();
}
