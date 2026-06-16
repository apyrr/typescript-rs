#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification_modules() {
    let mut t = TestingT;
    run_test_semantic_classification_modules(&mut t);
}

fn run_test_semantic_classification_modules(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticClassificationModules") {
        return;
    }
    let content = r"module /*0*/M {
    export var v;
    export interface /*1*/I {
    }
}

var x: /*2*/M./*3*/I = /*4*/M.v;
var y = /*5*/M;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "namespace.declaration".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.local".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "namespace".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "namespace".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "variable.local".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "namespace".to_string(),
                text: "M".to_string(),
            },
        ],
    );
    done();
}
