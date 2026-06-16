#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_expressions_in_tsx() {
    let mut t = TestingT;
    run_test_formatting_in_expressions_in_tsx(&mut t);
}

fn run_test_formatting_in_expressions_in_tsx(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: test.tsx
import * as React from "react";
<div
    autoComplete={(function () {
return true/*1*/
    })() }
    >
</div>"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ";");
    f.verify_current_line_content(t, "        return true;");
    done();
}
