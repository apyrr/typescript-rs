#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_paths_unicode4() {
    let mut t = TestingT;
    run_test_organize_imports_paths_unicode4(&mut t);
}

fn run_test_organize_imports_paths_unicode4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import * as Ab from "./Ab";
import * as _aB from "./_aB";
import * as aB from "./aB";
import * as _Ab from "./_Ab";

console.log(_aB, _Ab, aB, Ab);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import * as _Ab from "./_Ab";
import * as _aB from "./_aB";
import * as Ab from "./Ab";
import * as aB from "./aB";

console.log(_aB, _Ab, aB, Ab);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            organize_imports_case_first: lsutil::OrganizeImportsCaseFirst::Upper,
            ..Default::default()
        }),
    );
    f.verify_organize_imports(
        t,
        r#"import * as _aB from "./_aB";
import * as _Ab from "./_Ab";
import * as aB from "./aB";
import * as Ab from "./Ab";

console.log(_aB, _Ab, aB, Ab);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSFalse,
            organize_imports_collation: lsutil::OrganizeImportsCollation::Unicode,
            organize_imports_case_first: lsutil::OrganizeImportsCaseFirst::Lower,
            ..Default::default()
        }),
    );
    done();
}
