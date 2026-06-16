#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_constructor_find_all_references3() {
    let mut t = TestingT;
    run_test_constructor_find_all_references3(&mut t);
}

fn run_test_constructor_find_all_references3(t: &mut TestingT) {
    if should_skip_if_failing("TestConstructorFindAllReferences3") {
        return;
    }
    let content = r"export class C {
    /**/constructor() { }
    public foo() { }
}

new C().foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
