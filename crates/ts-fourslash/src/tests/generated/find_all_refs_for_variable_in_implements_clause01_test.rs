#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_variable_in_implements_clause01() {
    let mut t = TestingT;
    run_test_find_all_refs_for_variable_in_implements_clause01(&mut t);
}

fn run_test_find_all_refs_for_variable_in_implements_clause01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var Base = class { };
class C extends Base implements /**/Base { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
