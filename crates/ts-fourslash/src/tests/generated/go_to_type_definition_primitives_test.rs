#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_primitives() {
    let mut t = TestingT;
    run_test_go_to_type_definition_primitives(&mut t);
}

fn run_test_go_to_type_definition_primitives(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: module1.ts
var w: {a: number};
var x = "string";
var y: number | string;
var z; // any
// @Filename: module2.ts
w./*reference1*/a;
/*reference2*/x;
/*reference3*/y;
/*reference4*/y;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(
        t,
        &[
            "reference1".to_string(),
            "reference2".to_string(),
            "reference3".to_string(),
            "reference4".to_string(),
        ],
    );
    done();
}
