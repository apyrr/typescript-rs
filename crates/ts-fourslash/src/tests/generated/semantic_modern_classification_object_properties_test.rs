#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_object_properties() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_object_properties(&mut t);
}

fn run_test_semantic_modern_classification_object_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticModernClassificationObjectProperties") {
        return;
    }
    let content = r"let x = 1, y = 1;
const a1 = { e: 1 };
var a2 = { x };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.readonly".to_string(),
                text: "a1".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "e".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "a2".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "x".to_string(),
            },
        ],
    );
    done();
}
