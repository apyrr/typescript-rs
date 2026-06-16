#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_group_newline() {
    let mut t = TestingT;
    run_test_organize_imports_group_newline(&mut t);
}

fn run_test_organize_imports_group_newline(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import c from "C";

import d from "D";
import a from "A"; // not count
import b from "B";

console.log(a, b, c, d)"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import c from "C";

import a from "A"; // not count
import b from "B";
import d from "D";

console.log(a, b, c, d)"#,
        "source.organizeImports",
        None,
    );
    done();
}
