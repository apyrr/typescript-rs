#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_variable_in_extends_clause02() {
    let mut t = TestingT;
    run_test_find_all_refs_for_variable_in_extends_clause02(&mut t);
}

fn run_test_find_all_refs_for_variable_in_extends_clause02(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForVariableInExtendsClause02") {
        return;
    }
    let content = r"/*1*/interface /*2*/Base { }
namespace n {
    var Base = class { };
    interface I extends /*3*/Base { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
