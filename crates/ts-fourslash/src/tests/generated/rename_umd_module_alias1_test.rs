#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_umd_module_alias1() {
    let mut t = TestingT;
    run_test_rename_umd_module_alias1(&mut t);
}

fn run_test_rename_umd_module_alias1(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameUMDModuleAlias1") {
        return;
    }
    let content = r#"// @Filename: 0.d.ts
export function doThing(): string;
export function doTheOtherThing(): void;
[|export as namespace [|{| "contextRangeIndex": 0 |}myLib|];|]
// @Filename: 1.ts
/// <reference path="0.d.ts" />
[|myLib|].doThing();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "myLib");
    done();
}
