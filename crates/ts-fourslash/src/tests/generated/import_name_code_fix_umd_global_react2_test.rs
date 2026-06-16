#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_umd_global_react2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_umd_global_react2(&mut t);
}

fn run_test_import_name_code_fix_umd_global_react2(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixUMDGlobalReact2") {
        return;
    }
    let content = r"// @jsx: react
// @jsxFactory: factory
// @Filename: /factory.ts
export function factory() { return {}; }
declare global {
    namespace JSX {
        interface Element {}
    }
}
// @Filename: /a.tsx
[|<div/>|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { factory } from "./factory";

<div/>"#
                .to_string(),
        ],
        None,
    );
    done();
}
