#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_interfaces() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_interfaces(&mut t);
}

fn run_test_semantic_modern_classification_interfaces(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticModernClassificationInterfaces") {
        return;
    }
    let content = r"interface Pos { x: number, y: number };
const p = { x: 1, y: 2 } as Pos;
const foo = (o: Pos) => o.x + o.y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "Pos".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.readonly".to_string(),
                text: "p".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "Pos".to_string(),
            },
            SemanticToken {
                type_: "function.declaration.readonly".to_string(),
                text: "foo".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "o".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "Pos".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "o".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "o".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "y".to_string(),
            },
        ],
    );
    done();
}
