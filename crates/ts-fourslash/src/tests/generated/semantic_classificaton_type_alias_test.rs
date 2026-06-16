#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classificaton_type_alias() {
    let mut t = TestingT;
    run_test_semantic_classificaton_type_alias(&mut t);
}

fn run_test_semantic_classificaton_type_alias(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type /*0*/Alias = number
var x: /*1*/Alias;
var y = </*2*/Alias>{};
function f(x: /*3*/Alias): /*4*/Alias { return undefined; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "type.declaration".to_string(),
                text: "Alias".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "type".to_string(),
                text: "Alias".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "type".to_string(),
                text: "Alias".to_string(),
            },
            SemanticToken {
                type_: "function.declaration".to_string(),
                text: "f".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "type".to_string(),
                text: "Alias".to_string(),
            },
            SemanticToken {
                type_: "type".to_string(),
                text: "Alias".to_string(),
            },
        ],
    );
    done();
}
