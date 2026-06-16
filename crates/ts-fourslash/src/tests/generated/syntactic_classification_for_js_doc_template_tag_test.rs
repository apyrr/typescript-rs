#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classification_for_js_doc_template_tag() {
    let mut t = TestingT;
    run_test_syntactic_classification_for_js_doc_template_tag(&mut t);
}

fn run_test_syntactic_classification_for_js_doc_template_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassificationForJSDocTemplateTag") {
        return;
    }
    let content = r"/** @template T baring strait */
function ident<T>: T {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "function.declaration".to_string(),
                text: "ident".to_string(),
            },
            SemanticToken {
                type_: "typeParameter.declaration".to_string(),
                text: "T".to_string(),
            },
            SemanticToken {
                type_: "typeParameter".to_string(),
                text: "T".to_string(),
            },
        ],
    );
    done();
}
