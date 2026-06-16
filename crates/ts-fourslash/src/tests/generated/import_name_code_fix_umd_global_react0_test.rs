#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_umd_global_react0() {
    let mut t = TestingT;
    run_test_import_name_code_fix_umd_global_react0(&mut t);
}

fn run_test_import_name_code_fix_umd_global_react0(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixUMDGlobalReact0") {
        return;
    }
    let content = r#"// @jsx: react
// @allowSyntheticDefaultImports: false
// @module: es2015
// @moduleResolution: bundler
// @Filename: /node_modules/@types/react/index.d.ts
export = React;
export as namespace React;
declare namespace React {
    export class Component { render(): JSX.Element | null; }
}
declare global {
    namespace JSX {
        interface Element {}
    }
}
// @Filename: /a.tsx
[|import { Component } from "react";
export class MyMap extends Component { }
<MyMap/>;|]
// @Filename: /b.tsx
[|import { Component } from "react";
<></>;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import * as React from "react";
import { Component } from "react";
export class MyMap extends Component { }
<MyMap/>;"#
                .to_string(),
        ],
        None,
    );
    f.go_to_file(t, "/b.tsx");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import * as React from "react";
import { Component } from "react";
<></>;"#
                .to_string(),
        ],
        None,
    );
    done();
}
