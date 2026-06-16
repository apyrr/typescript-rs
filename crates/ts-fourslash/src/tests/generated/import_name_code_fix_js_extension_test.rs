#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_js_extension() {
    let mut t = TestingT;
    run_test_import_name_code_fix_js_extension(&mut t);
}

fn run_test_import_name_code_fix_js_extension(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_jsExtension") {
        return;
    }
    let content = r#"// @moduleResolution: bundler
// @noLib: true
// @jsx: preserve
// @Filename: /a.ts
export function a() {}
// @Filename: /b.ts
export function b() {}
// @Filename: /c.tsx
export function c() {}
// @Filename: /c.ts
import * as g from "global"; // Global imports skipped
import { a } from "./a.js";
import { a as a2 } from "./a"; // Ignored, only the first relative import is considered
b; c;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/c.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import * as g from "global"; // Global imports skipped
import { a } from "./a.js";
import { a as a2 } from "./a"; // Ignored, only the first relative import is considered
import { b } from "./b.js";
import { c } from "./c.jsx";
b; c;"#
                .to_string(),
        },
    );
    done();
}
