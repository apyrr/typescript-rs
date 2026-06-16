#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_remove_only() {
    let mut t = TestingT;
    run_test_organize_imports_remove_only(&mut t);
}

fn run_test_organize_imports_remove_only(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports_removeOnly") {
        return;
    }
    let content = r#"import { c, b, a } from "foo";
import d, { e } from "bar";
import * as f from "baz";
import { g } from "foo";

export { g, e, b, c };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import { c, b } from "foo";
import { e } from "bar";
import { g } from "foo";

export { g, e, b, c };"#,
        "source.removeUnusedImports",
        None,
    );
    done();
}
