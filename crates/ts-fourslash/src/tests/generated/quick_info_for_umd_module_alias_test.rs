#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_umd_module_alias() {
    let mut t = TestingT;
    run_test_quick_info_for_umd_module_alias(&mut t);
}

fn run_test_quick_info_for_umd_module_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForUMDModuleAlias") {
        return;
    }
    let content = r#"// @Filename: 0.d.ts
export function doThing(): string;
export function doTheOtherThing(): void;
export as namespace /*0*/myLib;
// @Filename: 1.ts
/// <reference path="0.d.ts" />
/*1*/myLib.doThing();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "0", "export namespace myLib", "");
    f.verify_quick_info_at(t, "1", "export namespace myLib", "");
    done();
}
