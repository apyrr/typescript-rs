#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_script_import() {
    let mut t = TestingT;
    run_test_go_to_definition_script_import(&mut t);
}

fn run_test_go_to_definition_script_import(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionScriptImport") {
        return;
    }
    let content = r#"// @filename: scriptThing.ts
/*1d*/console.log("woooo side effects")
// @filename: stylez.css
/*2d*/div {
  color: magenta;
}
// @filename: moduleThing.ts
import [|/*1*/"./scriptThing"|];
import [|/*2*/"./stylez.css"|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "2".to_string()]);
    done();
}
