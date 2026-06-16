#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_members() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_members(&mut t);
}

fn run_test_semantic_modern_classification_members(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticModernClassificationMembers") {
        return;
    }
    let content = r"class A {
  static x = 9;
  f = 9;
  async m() { return A.x + await this.m(); };
  get s() { return this.f; 
  static t() { return new A().f; };
  constructor() {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "A".to_string(),
            },
            SemanticToken {
                type_: "property.declaration.static".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "f".to_string(),
            },
            SemanticToken {
                type_: "method.declaration.async".to_string(),
                text: "m".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "A".to_string(),
            },
            SemanticToken {
                type_: "property.static".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "method.async".to_string(),
                text: "m".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "s".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "f".to_string(),
            },
            SemanticToken {
                type_: "method.declaration.static".to_string(),
                text: "t".to_string(),
            },
            SemanticToken {
                type_: "class".to_string(),
                text: "A".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "f".to_string(),
            },
        ],
    );
    done();
}
