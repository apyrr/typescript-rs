#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_unicode1() {
    let mut t = TestingT;
    run_test_organize_imports_unicode1(&mut t);
}

fn run_test_organize_imports_unicode1(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsUnicode1") {
        return;
    }
    let content = r"import {
    Ab,
    _aB,
    aB,
    _Ab,
} from './foo';

console.log(_aB, _Ab, aB, Ab);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import {
    Ab,
    _Ab,
    _aB,
    aB,
} from './foo';

console.log(_aB, _Ab, aB, Ab);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Ordinal,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r"import {
    _aB,
    _Ab,
    aB,
    Ab,
} from './foo';

console.log(_aB, _Ab, aB, Ab);",
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            ..Default::default()
        }),
    );
    done();
}
