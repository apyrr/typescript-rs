#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_script_import_server() {
    let mut t = TestingT;
    run_test_go_to_definition_script_import_server(&mut t);
}

fn run_test_go_to_definition_script_import_server(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionScriptImportServer") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: /home/src/workspaces/project/scriptThing.ts
/*1d*/console.log("woooo side effects")
// @Filename: /home/src/workspaces/project/stylez.css
/*2d*/div {
  color: magenta;
}
// @Filename: /home/src/workspaces/project/moduleThing.ts
import [|/*1*/"./scriptThing"|];
import [|/*2*/"./stylez.css"|];
import [|/*3*/"./foo.txt"|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
