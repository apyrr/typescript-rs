#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_imported_names7() {
    let mut t = TestingT;
    run_test_go_to_definition_imported_names7(&mut t);
}

fn run_test_go_to_definition_imported_names7(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: b.ts
import [|/*classAliasDefinition*/defaultExport|] from "./a";
// @Filename: a.ts
class /*classDefinition*/Class {
    private f;
}
export default Class;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["classAliasDefinition".to_string()]);
    done();
}
