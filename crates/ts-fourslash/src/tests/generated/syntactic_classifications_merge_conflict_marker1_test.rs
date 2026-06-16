#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_syntactic_classifications_merge_conflict_marker1() {
    let mut t = TestingT;
    run_test_syntactic_classifications_merge_conflict_marker1(&mut t);
}

fn run_test_syntactic_classifications_merge_conflict_marker1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"<<<<<<< HEAD
"AAAA"
=======
"BBBB"
>>>>>>> Feature"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(t, &[]);
    done();
}
