#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_paths_unicode3() {
    let mut t = TestingT;
    run_test_organize_imports_paths_unicode3(&mut t);
}

fn run_test_organize_imports_paths_unicode3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import * as B from "./B";
import * as À from "./À";
import * as A from "./A";

console.log(A, À, B);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import * as À from "./À";
import * as A from "./A";
import * as B from "./B";

console.log(A, À, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            organize_imports_accent_collation: core::TSFalse,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r#"import * as A from "./A";
import * as À from "./À";
import * as B from "./B";

console.log(A, À, B);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            organize_imports_accent_collation: core::TSTrue,
            ..Default::default()
        }),
    );
    done();
}
