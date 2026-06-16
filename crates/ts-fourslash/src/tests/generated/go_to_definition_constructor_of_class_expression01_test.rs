#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_constructor_of_class_expression01() {
    let mut t = TestingT;
    run_test_go_to_definition_constructor_of_class_expression01(&mut t);
}

fn run_test_go_to_definition_constructor_of_class_expression01(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionConstructorOfClassExpression01") {
        return;
    }
    let content = r"var x = class C {
    /*definition*/constructor() {
        var other = new [|/*xusage*/C|];
    }
}

var y = class C extends x {
    constructor() {
        super();
        var other = new [|/*yusage*/C|];
    }
}
var z = class C extends x {
    m() {
        return new [|/*zusage*/C|];
    }
}

var x1 = new [|/*cref*/C|]();
var x2 = new [|/*xref*/x|]();
var y1 = new [|/*yref*/y|]();
var z1 = new [|/*zref*/z|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "xusage".to_string(),
            "yusage".to_string(),
            "zusage".to_string(),
            "cref".to_string(),
            "xref".to_string(),
            "yref".to_string(),
            "zref".to_string(),
        ],
    );
    done();
}
