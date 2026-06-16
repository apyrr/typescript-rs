#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_constructor_types() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_constructor_types(&mut t);
}

fn run_test_semantic_modern_classification_constructor_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
Object.create(null);
const x = Promise.resolve(Number.MAX_VALUE);
if (x instanceof Promise) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "class.defaultLibrary".to_string(),
                text: "Object".to_string(),
            },
            SemanticToken {
                type_: "method.defaultLibrary".to_string(),
                text: "create".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.readonly".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "class.defaultLibrary".to_string(),
                text: "Number".to_string(),
            },
            SemanticToken {
                type_: "property.readonly.defaultLibrary".to_string(),
                text: "MAX_VALUE".to_string(),
            },
            SemanticToken {
                type_: "variable.readonly".to_string(),
                text: "x".to_string(),
            },
        ],
    );
    done();
}
