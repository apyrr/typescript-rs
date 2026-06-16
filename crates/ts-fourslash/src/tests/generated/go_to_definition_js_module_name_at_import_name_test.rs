#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_js_module_name_at_import_name() {
    let mut t = TestingT;
    run_test_go_to_definition_js_module_name_at_import_name(&mut t);
}

fn run_test_go_to_definition_js_module_name_at_import_name(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionJsModuleNameAtImportName") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: /foo.js
 /*moduleDef*/function notExported() { }
 class Blah {
    abc = 123;
 }
 module.exports.Blah = Blah;
// @Filename: /bar.js
const [|/*importDef*/BlahModule|] = require("./foo.js");
new [|/*importUsage*/BlahModule|].Blah()
// @Filename: /barTs.ts
import [|/*importDefTs*/BlahModule|] = require("./foo.js");
new [|/*importUsageTs*/BlahModule|].Blah()"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "importDef".to_string(),
            "importUsage".to_string(),
            "importDefTs".to_string(),
            "importUsageTs".to_string(),
        ],
    );
    done();
}
