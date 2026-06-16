#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_classification_alias() {
    let mut t = TestingT;
    run_test_semantic_classification_alias(&mut t);
}

fn run_test_semantic_classification_alias(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticClassificationAlias") {
        return;
    }
    let content = r#"// @Filename: /a.ts
export type x = number;
export class y {};
// @Filename: /b.ts
import { /*0*/x, /*1*/y } from "./a";
const v: /*2*/x = /*3*/y;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "variable.declaration.readonly".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "type".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "y".to_string(),
            },
        ],
    );
    done();
}
