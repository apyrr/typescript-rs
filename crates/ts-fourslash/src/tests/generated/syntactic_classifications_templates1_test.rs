#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_templates1() {
    let mut t = TestingT;
    run_test_syntactic_classifications_templates1(&mut t);
}

fn run_test_syntactic_classifications_templates1(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassificationsTemplates1") {
        return;
    }
    let content = r"var v = 10e0;
var x = {
    p1: `hello world`,
    p2: `goodbye ${0} cruel ${0} world`,
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "p1".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "p2".to_string(),
            },
        ],
    );
    done();
}
