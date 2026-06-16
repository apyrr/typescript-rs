#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_export_clause_error_reporting0() {
    let mut t = TestingT;
    run_test_export_clause_error_reporting0(&mut t);
}

fn run_test_export_clause_error_reporting0(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace M {
    /*1*/class C<T> { }
}
 
var x = new M.C<string>();
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.insert(t, "export ");
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 8);
    done();
}
