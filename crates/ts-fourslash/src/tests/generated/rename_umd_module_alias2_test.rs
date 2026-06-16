#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_umd_module_alias2() {
    let mut t = TestingT;
    run_test_rename_umd_module_alias2(&mut t);
}

fn run_test_rename_umd_module_alias2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: 0.d.ts
export function doThing(): string;
export function doTheOtherThing(): void;
export as namespace /**/[|myLib|];
// @Filename: 1.ts
/// <reference path="0.d.ts" />
myLib.doThing();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_succeeded_at_current_position();
    done();
}
