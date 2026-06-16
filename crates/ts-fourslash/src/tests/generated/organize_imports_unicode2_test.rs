#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_unicode2() {
    let mut t = TestingT;
    run_test_organize_imports_unicode2(&mut t);
}

fn run_test_organize_imports_unicode2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"import {
    a2,
    a100,
    a1,
} from './foo';

console.log(a1, a2, a100);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import {
    a1,
    a100,
    a2,
} from './foo';

console.log(a1, a2, a100);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            organize_imports_numeric_collation: core::TSFalse,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r"import {
    a1,
    a2,
    a100,
} from './foo';

console.log(a1, a2, a100);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            organize_imports_numeric_collation: core::TSTrue,
            ..Default::default()
        }),
    );
    done();
}
