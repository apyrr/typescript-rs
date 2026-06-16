#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_for_of_keyword2() {
    let mut t = TestingT;
    run_test_syntactic_classifications_for_of_keyword2(&mut t);
}

fn run_test_syntactic_classifications_for_of_keyword2(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassificationsForOfKeyword2") {
        return;
    }
    let content = r"for (var of in of) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "of".to_string(),
            },
            SemanticToken {
                type_: "variable".to_string(),
                text: "of".to_string(),
            },
        ],
    );
    done();
}
