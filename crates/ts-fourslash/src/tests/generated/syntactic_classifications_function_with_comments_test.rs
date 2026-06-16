#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_function_with_comments() {
    let mut t = TestingT;
    run_test_syntactic_classifications_function_with_comments(&mut t);
}

fn run_test_syntactic_classifications_function_with_comments(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassificationsFunctionWithComments") {
        return;
    }
    let content = r"/**
 * This is my function.
 * There are many like it, but this one is mine.
 */
function myFunction(/* x */ x: any) {
    var y = x ? x++ : ++x;
}
// end of file";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "function.declaration".to_string(),
                text: "myFunction".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.local".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "x".to_string(),
            },
        ],
    );
    done();
}
