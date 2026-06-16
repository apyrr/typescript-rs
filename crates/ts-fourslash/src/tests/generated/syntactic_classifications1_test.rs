#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications1() {
    let mut t = TestingT;
    run_test_syntactic_classifications1(&mut t);
}

fn run_test_syntactic_classifications1(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntacticClassifications1") {
        return;
    }
    let content = r#"// comment
namespace M {
    var v = 0 + 1;
    var s = "string";

    class C<T> {
    }

    enum E {
    }

    interface I {
    }

    namespace M1.M2 {
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            SemanticToken {
                type_: "namespace.declaration".to_string(),
                text: "M".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.local".to_string(),
                text: "v".to_string(),
            },
            SemanticToken {
                type_: "variable.declaration.local".to_string(),
                text: "s".to_string(),
            },
            SemanticToken {
                type_: "class.declaration".to_string(),
                text: "C".to_string(),
            },
            SemanticToken {
                type_: "typeParameter.declaration".to_string(),
                text: "T".to_string(),
            },
            SemanticToken {
                type_: "enum.declaration".to_string(),
                text: "E".to_string(),
            },
            SemanticToken {
                type_: "interface.declaration".to_string(),
                text: "I".to_string(),
            },
            SemanticToken {
                type_: "namespace.declaration".to_string(),
                text: "M1".to_string(),
            },
            SemanticToken {
                type_: "namespace.declaration".to_string(),
                text: "M2".to_string(),
            },
        ],
    );
    done();
}
