#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_items_module_variables() {
    let mut t = TestingT;
    run_test_navigation_bar_items_items_module_variables(&mut t);
}

fn run_test_navigation_bar_items_items_module_variables(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsItemsModuleVariables") {
        return;
    }
    let content = r"// @Filename: navigationItemsModuleVariables_0.ts
 /*file1*/
namespace Module1 {
    export var x = 0;
}
// @Filename: navigationItemsModuleVariables_1.ts
 /*file2*/
namespace Module1.SubModule {
    export var y = 0;
}
// @Filename: navigationItemsModuleVariables_2.ts
 /*file3*/
namespace Module1 {
    export var z = 0;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "file1");
    f.verify_baseline_document_symbol(t);
    f.go_to_marker(t, "file2");
    f.verify_baseline_document_symbol(t);
    done();
}
