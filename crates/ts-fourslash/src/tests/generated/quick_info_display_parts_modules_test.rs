#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_modules() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_modules(&mut t);
}

fn run_test_quick_info_display_parts_modules(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsModules") {
        return;
    }
    let content = r"namespace /*1*/m {
    var /*2*/namespaceElemWithoutExport = 10;
    export var /*3*/namespaceElemWithExport = 10;
}
var /*4*/a = /*5*/m;
var /*6*/b: typeof /*7*/m;
namespace /*8*/m1./*9*/m2 {
    var /*10*/namespaceElemWithoutExport = 10;
    export var /*11*/namespaceElemWithExport = 10;
}
var /*12*/x = /*13*/m1./*14*/m2;
var /*15*/y: typeof /*16*/m1./*17*/m2;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
