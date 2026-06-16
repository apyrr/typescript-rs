#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_expression_keywords() {
    let mut t = TestingT;
    run_test_references_for_expression_keywords(&mut t);
}

fn run_test_references_for_expression_keywords(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForExpressionKeywords") {
        return;
    }
    let content = r#"class C {
    static x = 1;
}
/*new*/new C();
/*void*/void C;
/*typeof*/typeof C;
/*delete*/delete C.x;
/*async*/async function* f() {
    /*yield*/yield C;
    /*await*/await C;
}
"x" /*in*/in C;
undefined /*instanceof*/instanceof C;
undefined /*as*/as C;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "new".to_string(),
            "void".to_string(),
            "typeof".to_string(),
            "yield".to_string(),
            "await".to_string(),
            "in".to_string(),
            "instanceof".to_string(),
            "as".to_string(),
            "delete".to_string(),
        ],
    );
    done();
}
