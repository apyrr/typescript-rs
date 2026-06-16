#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_imported_names3() {
    let mut t = TestingT;
    run_test_go_to_definition_imported_names3(&mut t);
}

fn run_test_go_to_definition_imported_names3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: e.ts
 import {M, [|/*classAliasDefinition*/C|], I} from "./d";
 var c = new [|/*classReference*/C|]();
// @Filename: d.ts
export * from "./c";
// @Filename: c.ts
export {Module as M, Class as C, Interface as I} from "./b";
// @Filename: b.ts
export * from "./a";
// @Filename: a.ts
export namespace Module {
}
export class /*classDefinition*/Class {
    private f;
}
export interface Interface {
    x;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "classReference".to_string(),
            "classAliasDefinition".to_string(),
        ],
    );
    done();
}
