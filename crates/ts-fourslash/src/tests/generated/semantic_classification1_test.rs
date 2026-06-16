#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification1() {
    let mut t = TestingT;
    run_test_semantic_classification1(&mut t);
}

fn run_test_semantic_classification1(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticClassification1") {
        return;
    }
    let content = r"module /*0*/M {
    export interface /*1*/I {
    }
}
interface /*2*/X extends /*3*/M./*4*/I { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "namespace.declaration".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "X".to_string(),
            },
            SemanticToken {
                type_: "namespace".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "I".to_string(),
            },
        ],
    );
    done();
}
