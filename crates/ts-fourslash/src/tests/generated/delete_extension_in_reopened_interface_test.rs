#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_delete_extension_in_reopened_interface() {
    let mut t = TestingT;
    run_test_delete_extension_in_reopened_interface(&mut t);
}

fn run_test_delete_extension_in_reopened_interface(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface A { a: number; }
interface B { b: number; }

interface I /*del*/extends A { }
interface I extends B { }

var i: I;
class C /*delImplements*/implements A { }
var c: C;
c.a;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "del");
    f.delete_at_caret(t, 9);
    f.go_to_eof(t);
    f.insert(t, "var a = i.a;");
    f.go_to_marker(t, "delImplements");
    f.delete_at_caret(t, 12);
    f.go_to_marker(t, "del");
    f.insert(t, "extends A");
    done();
}
