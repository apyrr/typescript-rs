#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_jsx_react17() {
    let mut t = TestingT;
    run_test_import_name_code_fix_jsx_react17(&mut t);
}

fn run_test_import_name_code_fix_jsx_react17(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @jsx: preserve
// @module: commonjs
// @Filename: /node_modules/@types/react/index.d.ts
declare namespace React {
  function createElement(): any;
}
export = React;
export as namespace React;

declare global {
  namespace JSX {
    interface IntrinsicElements {}
    interface IntrinsicAttributes {}
  }  
}
// @Filename: /component.tsx
import "react";
export declare function Component(): any;
// @Filename: /index.tsx
(<Component/**/ />);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Component } from "./component";

(<Component />);"#
                .to_string(),
        ],
        None,
    );
    done();
}
