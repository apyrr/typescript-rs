#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_export_from_interface_error1() {
    let mut t = TestingT;
    run_test_remove_export_from_interface_error1(&mut t);
}

fn run_test_remove_export_from_interface_error1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace M {
export class C1 { }
    /*1*/export interface I { n: number; }
}
namespace M {
function f(): I { return null; } }
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 6);
    done();
}
