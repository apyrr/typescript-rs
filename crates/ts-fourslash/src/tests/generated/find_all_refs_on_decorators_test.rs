#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_on_decorators() {
    let mut t = TestingT;
    run_test_find_all_refs_on_decorators(&mut t);
}

fn run_test_find_all_refs_on_decorators(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a.ts
/*1*/function /*2*/decorator(target) {
    return target;
}
/*3*/decorator();
// @Filename: b.ts
@/*4*/decorator @/*5*/decorator("again")
class C {
    @/*6*/decorator
    method() {}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
        ],
    );
    done();
}
