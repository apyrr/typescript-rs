#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_type3() {
    let mut t = TestingT;
    run_test_organize_imports_type3(&mut t);
}

fn run_test_organize_imports_type3(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsType3") {
        return;
    }
    let content = r"import {
    d, 
    type d as D,
    type c,
    c as C,
    b,
    b as B,
    type A,
    a
} from './foo';
console.log(A, a, B, b, c, C, d, D);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import {
    type A,
    b as B,
    c as C,
    type d as D,
    a,
    b,
    type c,
    d
} from './foo';
console.log(A, a, B, b, c, C, d, D);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    done();
}
