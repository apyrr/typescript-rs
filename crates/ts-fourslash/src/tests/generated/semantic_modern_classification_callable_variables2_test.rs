#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_callable_variables2() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_callable_variables2(&mut t);
}

fn run_test_semantic_modern_classification_callable_variables2(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticModernClassificationCallableVariables2") {
        return;
    }
    let content = r#"import "node";
var fs = require("fs")
require.resolve('react');
require.resolve.paths;
interface LanguageMode { getFoldingRanges?: (d: string) => number[]; };
function (mode: LanguageMode | undefined) { if (mode && mode.getFoldingRanges) { return mode.getFoldingRanges('a'); }};
function b(a: () => void) { a(); };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "fs".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "LanguageMode".to_string(),
            },
            SemanticToken {
                type_: "method.declaration".to_string(),
                text: "getFoldingRanges".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "d".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "mode".to_string(),
            },
            SemanticToken {
                type_: "interface".to_string(),
                text: "LanguageMode".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "mode".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "mode".to_string(),
            },
            SemanticToken {
                type_: "method".to_string(),
                text: "getFoldingRanges".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "mode".to_string(),
            },
            SemanticToken {
                type_: "method".to_string(),
                text: "getFoldingRanges".to_string(),
            },
            SemanticToken {
                type_: "function.declaration".to_string(),
                text: "b".to_string(),
            },
            SemanticToken {
                type_: "function.declaration".to_string(),
                text: "a".to_string(),
            },
            SemanticToken {
                type_: "function".to_string(),
                text: "a".to_string(),
            },
        ],
    );
    done();
}
