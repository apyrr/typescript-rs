#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports8() {
    let mut t = TestingT;
    run_test_organize_imports8(&mut t);
}

fn run_test_organize_imports8(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports8") {
        return;
    }
    let content = r#"import { foo as foo } from "foo";
foo;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { foo } from "foo";
foo;"#,
        "source.organizeImports",
        None,
    );
    done();
}
