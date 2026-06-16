#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_module_to_var() {
    let mut t = TestingT;
    run_test_rename_module_to_var(&mut t);
}

fn run_test_rename_module_to_var(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameModuleToVar") {
        return;
    }
    let content = r"interface IMod {
    y: number;
}
declare module/**/ X: IMod;// {
//    export var y: numb;
var y: number;
namespace Y {
    var z = y + 5;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.backspace(t, 6);
    f.insert(t, "var");
    f.verify_no_errors();
    done();
}
