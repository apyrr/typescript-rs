#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_imported_names6() {
    let mut t = TestingT;
    run_test_go_to_definition_imported_names6(&mut t);
}

fn run_test_go_to_definition_imported_names6(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionImportedNames6") {
        return;
    }
    let content = r#"// @Filename: b.ts
import [|/*moduleAliasDefinition*/alias|] = require("./a");
// @Filename: a.ts
/*moduleDefinition*/export namespace Module {
}
export class Class {
    private f;
}
export interface Interface {
    x;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["moduleAliasDefinition".to_string()]);
    done();
}
