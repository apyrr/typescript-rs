#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_external_module_alias() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_external_module_alias(&mut t);
}

fn run_test_quick_info_display_parts_external_module_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsExternalModuleAlias") {
        return;
    }
    let content = r#"// @Filename: quickInfoDisplayPartsExternalModuleAlias_file0.ts
export namespace m1 {
    export class c {
    }
}
// @Filename: quickInfoDisplayPartsExternalModuleAlias_file1.ts
import /*1*/a1 = require(/*mod1*/"./quickInfoDisplayPartsExternalModuleAlias_file0");
new /*2*/a1.m1.c();
export import /*3*/a2 = require(/*mod2*/"./quickInfoDisplayPartsExternalModuleAlias_file0");
new /*4*/a2.m1.c();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
