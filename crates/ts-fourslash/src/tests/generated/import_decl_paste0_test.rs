#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_decl_paste0() {
    let mut t = TestingT;
    run_test_import_decl_paste0(&mut t);
}

fn run_test_import_decl_paste0(t: &mut TestingT) {
    if should_skip_if_failing("TestImportDeclPaste0") {
        return;
    }
    let content = r"// @Filename: exportEqualsInterface_A.ts
interface A {
	p1: number;
}

export = A;
/*1*/
var i: I1;

var n: number = i.p1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    done();
}
