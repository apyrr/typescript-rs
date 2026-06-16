#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_variables() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_variables(&mut t);
}

fn run_test_semantic_modern_classification_variables(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"  var x = 9, y1 = [x];
  try {
    for (const s of y1) { x = s }
  } catch (e) {
    throw y1;
  }";
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
                text: "y1".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.readonly.local".to_string(),
                text: "s".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "y1".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly.local".to_string(),
                text: "s".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.local".to_string(),
                text: "e".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "y1".to_string(),
            },
        ],
    );
    done();
}
