#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_object_literal() {
    let mut t = TestingT;
    run_test_syntactic_classifications_object_literal(&mut t);
}

fn run_test_syntactic_classifications_object_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var v = 10e0;
var x = {
    p1: 1,
    p2: 2,
    any: 3,
    function: 4,
    var: 5,
    void: void 0,
    v: v += v,
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "p1".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "p2".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "any".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "function".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "var".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "void".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "v".to_string(),
            },
        ],
    );
    done();
}
