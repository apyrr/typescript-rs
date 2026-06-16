#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_doc_comment4() {
    let mut t = TestingT;
    run_test_syntactic_classifications_doc_comment4(&mut t);
}

fn run_test_syntactic_classifications_doc_comment4(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassificationsDocComment4") {
        return;
    }
    let content = r"/** @param {number} p1 */
function foo(p1) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "function.declaration".to_string(),
                text: "foo".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "p1".to_string(),
            },
        ],
    );
    done();
}
