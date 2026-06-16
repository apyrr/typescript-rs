#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_global_module_augmentation() {
    let mut t = TestingT;
    run_test_find_all_refs_global_module_augmentation(&mut t);
}

fn run_test_find_all_refs_global_module_augmentation(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsGlobalModuleAugmentation") {
        return;
    }
    let content = r"// @Filename: /a.ts
export {};
declare global {
    /*1*/function /*2*/f(): void;
}
// @Filename: /b.ts
/*3*/f();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
