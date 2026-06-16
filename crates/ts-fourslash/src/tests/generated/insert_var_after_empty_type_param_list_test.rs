#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_insert_var_after_empty_type_param_list() {
    let mut t = TestingT;
    run_test_insert_var_after_empty_type_param_list(&mut t);
}

fn run_test_insert_var_after_empty_type_param_list(t: &mut TestingT) {
    if should_skip_if_failing("TestInsertVarAfterEmptyTypeParamList") {
        return;
    }
    let content = r"class Dictionary<> { }
var x;
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "var y;\n");
    done();
}
