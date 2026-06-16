#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_callable_variables() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_callable_variables(&mut t);
}

fn run_test_semantic_modern_classification_callable_variables(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class A { onEvent: () => void; }
const x = new A().onEvent;
const match = (s: any) => x();
const other = match;
match({ other });
interface B = { (): string; }; var b: B
var s: String;
var t: { (): string; foo: string};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "A".to_string(),
            },
            SemanticToken {
                type_: "method.declaration".to_string(),
                text: "onEvent".to_string(),
            },
            SemanticToken {
                type_: "function.declaration.readonly".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "A".to_string(),
            },
            SemanticToken {
                type_: "method".to_string(),
                text: "onEvent".to_string(),
            },
            SemanticToken {
                type_: "function.declaration.readonly".to_string(),
                text: "match".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "s".to_string(),
            },
            SemanticToken {
                type_: "function.readonly".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "function.declaration.readonly".to_string(),
                text: "other".to_string(),
            },
            SemanticToken {
                type_: "function.readonly".to_string(),
                text: "match".to_string(),
            },
            SemanticToken {
                type_: "function.readonly".to_string(),
                text: "match".to_string(),
            },
            SemanticToken {
                type_: "method.declaration".to_string(),
                text: "other".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "B".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "b".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "B".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "s".to_string(),
            },
            SemanticToken {
                type_: "interface.defaultLibrary".to_string(),
                text: "String".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "t".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "foo".to_string(),
            },
        ],
    );
    done();
}
