#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_infinity_and_na_n() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_infinity_and_na_n(&mut t);
}

fn run_test_semantic_modern_classification_infinity_and_na_n(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticModernClassificationInfinityAndNaN") {
        return;
    }
    let content = r#" Infinity;
 NaN;

// Regular properties

const obj1 = {
    Infinity: 100,
    NaN: 200,
    "-Infinity": 300
};

obj1.Infinity;
obj1.NaN;
obj1["-Infinity"];

// Shorthand properties

const obj2 = {
    Infinity,
    NaN,
}

obj2.Infinity;
obj2.NaN;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration.readonly".to_string(),
                text: "obj1".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly".to_string(),
                text: "obj1".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly".to_string(),
                text: "obj1".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly".to_string(),
                text: "obj1".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.readonly".to_string(),
                text: "obj2".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly".to_string(),
                text: "obj2".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly".to_string(),
                text: "obj2".to_string(),
            },
        ],
    );
    done();
}
