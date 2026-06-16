#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification2() {
    let mut t = TestingT;
    run_test_semantic_classification2(&mut t);
}

fn run_test_semantic_classification2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface /*0*/Thing {
    toExponential(): number;
}

var Thing = 0;
Thing.toExponential();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "Thing".to_string(),
            },
            SemanticToken {
                type_: "method.declaration".to_string(),
                text: "toExponential".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "Thing".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "Thing".to_string(),
            },
            SemanticToken {
                type_: "method.defaultLibrary".to_string(),
                text: "toExponential".to_string(),
            },
        ],
    );
    done();
}
