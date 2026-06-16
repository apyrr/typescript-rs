#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_refactor_convert_to_es_module_module_node12() {
    let mut t = TestingT;
    run_test_refactor_convert_to_es_module_module_node12(&mut t);
}

fn run_test_refactor_convert_to_es_module_module_node12(t: &mut TestingT) {
    if should_skip_if_failing("TestRefactorConvertToEsModule_module_node12") {
        return;
    }
    let content = r"// @allowJs: true
// @target: esnext
// @module: node16
// @Filename: /a.js
module.exports = 0;
// @Filename: /b.ts
module.exports = 0;
// @Filename: /c.cjs
module.exports = 0;
// @Filename: /d.cts
module.exports = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.js");
    f.verify_code_fix_not_available(t, &[]);
    f.go_to_file(t, "/b.ts");
    f.verify_code_fix_not_available(t, &[]);
    f.go_to_file(t, "/c.cjs");
    f.verify_code_fix_not_available(t, &[]);
    f.go_to_file(t, "/d.cts");
    f.verify_code_fix_not_available(t, &[]);
    done();
}
