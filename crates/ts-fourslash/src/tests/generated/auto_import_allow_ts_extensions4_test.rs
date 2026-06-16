#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_allow_ts_extensions4() {
    let mut t = TestingT;
    run_test_auto_import_allow_ts_extensions4(&mut t);
}

fn run_test_auto_import_allow_ts_extensions4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @moduleResolution: bundler
// @allowImportingTsExtensions: true
// @noEmit: true
// @Filename: /local.ts
export const fromLocal: number;
// @Filename: /decl.d.ts
export const fromDecl: number;
// @Filename: /Component.tsx
export function Component() { return null; }
// @Filename: /main.ts
import { Component } from "./local.js";
/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.baseline_auto_imports_completions(t, &[]);
    done();
}
