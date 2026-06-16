#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_conflict_markers2() {
    let mut t = TestingT;
    run_test_syntactic_classifications_conflict_markers2(&mut t);
}

fn run_test_syntactic_classifications_conflict_markers2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"<<<<<<< HEAD
class C { }
=======
class D { }
>>>>>>> Branch - a";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[SemanticToken {
            type_: "class.declaration".to_string(),
            text: "C".to_string(),
        }],
    );
    done();
}
