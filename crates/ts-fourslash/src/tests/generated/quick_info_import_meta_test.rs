#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_import_meta() {
    let mut t = TestingT;
    run_test_quick_info_import_meta(&mut t);
}

fn run_test_quick_info_import_meta(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoImportMeta") {
        return;
    }
    let content = r"// @module: esnext
// @Filename: foo.ts
/// <reference path='./bar.d.ts' />
im/*1*/port.me/*2*/ta;
//@Filename: bar.d.ts
/**
 * The type of `import.meta`.
 *
 * If you need to declare that a given property exists on `import.meta`,
 * this type may be augmented via interface merging.
 */
 interface ImportMeta {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
