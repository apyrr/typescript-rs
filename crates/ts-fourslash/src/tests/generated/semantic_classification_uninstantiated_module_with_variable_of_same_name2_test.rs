#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification_uninstantiated_module_with_variable_of_same_name2() {
    let mut t = TestingT;
    run_test_semantic_classification_uninstantiated_module_with_variable_of_same_name2(&mut t);
}

fn run_test_semantic_classification_uninstantiated_module_with_variable_of_same_name2(
    t: &mut TestingT,
) {
    if should_skip_if_failing(
        "TestSemanticClassificationUninstantiatedModuleWithVariableOfSameName2",
    ) {
        return;
    }
    let content = r"module /*0*/M {
    export interface /*1*/I {
    }
}

var /*2*/M = {
    foo: 10,
    bar: 20
}

var v: /*3*/M./*4*/I;

var x = /*5*/M;";
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
                text: "foo".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "bar".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "v".to_string(),
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
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "M".to_string(),
            },
        ],
    );
    done();
}
