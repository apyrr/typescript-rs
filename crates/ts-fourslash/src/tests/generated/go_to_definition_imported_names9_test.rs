#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_imported_names9() {
    let mut t = TestingT;
    run_test_go_to_definition_imported_names9(&mut t);
}

fn run_test_go_to_definition_imported_names9(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionImportedNames9") {
        return;
    }
    let content = r#"// @allowjs: true
// @Filename: a.js
class /*classDefinition*/Class {
    f;
}
 export { Class };
// @Filename: b.js
const { Class } = require("./a");
 [|/*classAliasDefinition*/Class|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["classAliasDefinition".to_string()]);
    done();
}
