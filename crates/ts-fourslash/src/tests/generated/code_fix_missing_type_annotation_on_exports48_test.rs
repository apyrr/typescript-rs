#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports48() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports48(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports48(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @moduleResolution: bundler
// @target: es2018
// @jsx: react-jsx
// @filename: node_modules/react/package.json
{
    "name": "react",
    "types": "index.d.ts"
}
// @filename: node_modules/react/index.d.ts
export = React;
declare namespace JSX {
    interface Element extends GlobalJSXElement { }
    interface IntrinsicElements extends GlobalJSXIntrinsicElements { }
}
declare namespace React { }
declare global {
    namespace JSX {
        interface Element { }
        interface IntrinsicElements { [x: string]: any; }
    }
}
interface GlobalJSXElement extends JSX.Element {}
interface GlobalJSXIntrinsicElements extends JSX.IntrinsicElements {}
// @filename: node_modules/react/jsx-runtime.d.ts
import './';
// @filename: node_modules/react/jsx-dev-runtime.d.ts
import './';
// @filename: /a.tsx
export const x = <div aria-label="label text" />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_code_fix(t, VerifyCodeFixOptions {
    description: "Add satisfies and an inline type assertion with 'JSX.Element'".to_string(),
    new_file_content: r#"export const x = (<div aria-label="label text" />) satisfies JSX.Element as JSX.Element;"#.to_string(),
    new_range_content: String::new(),
    index: 1,
    apply_changes: false,
    user_preferences: None,
});
    done();
}
