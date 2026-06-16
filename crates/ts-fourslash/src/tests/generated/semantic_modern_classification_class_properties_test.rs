#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semantic_modern_classification_class_properties() {
    let mut t = TestingT;
    run_test_semantic_modern_classification_class_properties(&mut t);
}

fn run_test_semantic_modern_classification_class_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestSemanticModernClassificationClassProperties") {
        return;
    }
    let content = r"class A { 
  private y: number;
  constructor(public x : number, _y : number) { this.y = _y; }
  get z() : number { return this.x + this.y; }
  set a(v: number) { }
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
                type_: "property.declaration".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "_y".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "parameter".to_string(),
                text: "_y".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "z".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "x".to_string(),
            },
            SemanticToken {
                type_: "property".to_string(),
                text: "y".to_string(),
            },
            SemanticToken {
                type_: "property.declaration".to_string(),
                text: "a".to_string(),
            },
            SemanticToken {
                type_: "parameter.declaration".to_string(),
                text: "v".to_string(),
            },
        ],
    );
    done();
}
