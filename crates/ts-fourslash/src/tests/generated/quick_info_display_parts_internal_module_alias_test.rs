#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_internal_module_alias() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_internal_module_alias(&mut t);
}

fn run_test_quick_info_display_parts_internal_module_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsInternalModuleAlias") {
        return;
    }
    let content = r"namespace m.m1 {
    export class c {
    }
}
namespace m2 {
    import /*1*/a1 = m;
    new /*2*/a1.m1.c();
    import /*3*/a2 = m.m1;
    new /*4*/a2.c();
    export import /*5*/a3 = m;
    new /*6*/a3.m1.c();
    export import /*7*/a4 = m.m1;
    new /*8*/a4.c();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
