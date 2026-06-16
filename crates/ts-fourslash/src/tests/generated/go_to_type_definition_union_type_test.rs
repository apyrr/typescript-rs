#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_union_type() {
    let mut t = TestingT;
    run_test_go_to_type_definition_union_type(&mut t);
}

fn run_test_go_to_type_definition_union_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class /*definition0*/C {
    p;
}

interface /*definition1*/I {
    x;
}

namespace M {
    export interface /*definition2*/I {
        y;
    }
}

var x: C | I | M.I;

/*reference*/x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["reference".to_string()]);
    done();
}
