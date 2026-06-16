#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_react_jsx() {
    let mut t = TestingT;
    run_test_organize_imports_react_jsx(&mut t);
}

fn run_test_organize_imports_react_jsx(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowSyntheticDefaultImports: true
// @moduleResolution: bundler
// @noUnusedLocals: true
// @target: es2018
// @jsx: react-jsx
// @filename: test.tsx
import React from 'react';
export default () => <div></div>
// @filename: node_modules/react/package.json
{
    "name": "react",
    "types": "index.d.ts"
}
// @filename: node_modules/react/index.d.ts
export = React;
declare namespace JSX {
    interface IntrinsicElements { [x: string]: any; }
}
declare namespace React {}
// @filename: node_modules/react/jsx-runtime.d.ts
import './';
// @filename: node_modules/react/jsx-dev-runtime.d.ts
import './';"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "test.tsx");
    f.verify_organize_imports(
        t,
        r"export default () => <div></div>",
        "source.organizeImports",
        None,
    );
    done();
}
