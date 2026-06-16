#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type6() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type6(&mut t);
}

fn run_test_import_name_code_fix_import_type6(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: es2015
// @esModuleInterop: true
// @jsx: react
// @Filename: /types.d.ts
declare module "react" { var React: any; export = React; export as namespace React; }
// @Filename: /a.tsx
import type React from "react";
function Component() {}
(<Component/**/ />)"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import React from "react";
function Component() {}
(<Component />)"#
                .to_string(),
        ],
        None,
    );
    done();
}
