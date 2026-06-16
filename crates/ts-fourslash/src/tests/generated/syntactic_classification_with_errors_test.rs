#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classification_with_errors() {
    let mut t = TestingT;
    run_test_syntactic_classification_with_errors(&mut t);
}

fn run_test_syntactic_classification_with_errors(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class A {
    a:
}
c =";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "A".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "a".to_string(),
            },
        ],
    );
    done();
}
