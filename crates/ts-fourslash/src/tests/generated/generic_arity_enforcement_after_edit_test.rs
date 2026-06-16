#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_arity_enforcement_after_edit() {
    let mut t = TestingT;
    run_test_generic_arity_enforcement_after_edit(&mut t);
}

fn run_test_generic_arity_enforcement_after_edit(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface G<T, U> { }
/**/
var v4: G<G<any>, any>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "");
    f.insert(t, " ");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
