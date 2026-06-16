#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification_uninstantiated_module_with_variable_of_same_name1() {
    let mut t = TestingT;
    run_test_semantic_classification_uninstantiated_module_with_variable_of_same_name1(&mut t);
}

fn run_test_semantic_classification_uninstantiated_module_with_variable_of_same_name1(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"declare module /*0*/M {
    interface /*1*/I {

    }
}

var M = { I: 10 };";
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
                type_: "variable.declaration".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "I".to_string(),
            },
        ],
    );
    done();
}
