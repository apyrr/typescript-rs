#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification_with_union_types() {
    let mut t = TestingT;
    run_test_semantic_classification_with_union_types(&mut t);
}

fn run_test_semantic_classification_with_union_types(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticClassificationWithUnionTypes") {
        return;
    }
    let content = r"module /*0*/M {
    export interface /*1*/I {
    }
}

interface /*2*/I {
}
class /*3*/C {
}

var M: /*4*/M./*5*/I | /*6*/I | /*7*/C;
var I: typeof M | typeof /*8*/C;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "C".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "C".to_string(),
            },
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "C".to_string(),
            },
        ],
    );
    done();
}
