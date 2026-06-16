#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification_in_template_expressions() {
    let mut t = TestingT;
    run_test_semantic_classification_in_template_expressions(&mut t);
}

fn run_test_semantic_classification_in_template_expressions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"module /*0*/M {
    export class /*1*/C {
        static x;
    }
    export enum /*2*/E {
        E1 = 0
    }
}
` + "`" + `abcd${ /*3*/M./*4*/C.x + /*5*/M./*6*/E.E1}efg` + "`" + `"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "namespace.declaration".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "C".to_string(),
            },
            SemanticToken {
                type_: "property.declaration.static".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "enum.declaration".to_string(),
                text: "E".to_string(),
            },
            SemanticToken {
                type_: "enumMember.declaration.readonly".to_string(),
                text: "E1".to_string(),
            },
            SemanticToken {
                type_: "namespace".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "C".to_string(),
            },
            SemanticToken {
                type_: "property.static".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "namespace".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "enum".to_string(),
                text: "E".to_string(),
            },
            SemanticToken {
                type_: "enumMember.readonly".to_string(),
                text: "E1".to_string(),
            },
        ],
    );
    done();
}
