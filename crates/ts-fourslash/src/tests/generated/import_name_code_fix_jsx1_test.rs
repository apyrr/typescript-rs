#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_jsx1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_jsx1(&mut t);
}

fn run_test_import_name_code_fix_jsx1(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_jsx1") {
        return;
    }
    let content = r#"// @jsx: react
// @Filename: /node_modules/react/index.d.ts
export const React: any;
// @Filename: /a.tsx
[|<this>|]</this>
// @Filename: /Foo.tsx
export const Foo = 0;
// @Filename: /c.tsx
import { React } from "react";
<Foo />;
// @Filename: /d.tsx
import { Foo } from "./Foo";
<Foo />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_import_fix_at_position(t, &[], None);
    f.go_to_file(t, "/c.tsx");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { React } from "react";
import { Foo } from "./Foo";
<Foo />;"#
                .to_string(),
        ],
        None,
    );
    f.go_to_file(t, "/d.tsx");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { React } from "react";
import { Foo } from "./Foo";
<Foo />;"#
                .to_string(),
        ],
        None,
    );
    done();
}
