#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_jsx_opening_tag_import_default() {
    let mut t = TestingT;
    run_test_import_name_code_fix_jsx_opening_tag_import_default(&mut t);
}

fn run_test_import_name_code_fix_jsx_opening_tag_import_default(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: commonjs
// @jsx: react-jsx
// @Filename: /component.tsx
export default function (props: any) {}
// @Filename: /index.tsx
export function Index() {
    return <Component/**/ />;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import Component from "./component";

export function Index() {
    return <Component />;
}"#
            .to_string(),
        ],
        None,
    );
    done();
}
