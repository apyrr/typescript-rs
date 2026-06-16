#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_jsx2() {
    let mut t = TestingT;
    run_test_syntactic_classifications_jsx2(&mut t);
}

fn run_test_syntactic_classifications_jsx2(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassificationsJsx2") {
        return;
    }
    let content = r#"// @Filename: file1.tsx
let x  = <div.name b = "some-value" c = {1}>
    some jsx text
</div.name>;

let y = <element.name attr="123"/>"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration".to_string(),
                text: "y".to_string(),
            },
        ],
    );
    done();
}
