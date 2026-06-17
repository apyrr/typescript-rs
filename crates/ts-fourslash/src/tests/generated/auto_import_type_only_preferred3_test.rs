#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_type_only_preferred3() {
    let mut t = TestingT;
    run_test_auto_import_type_only_preferred3(&mut t);
}

fn run_test_auto_import_type_only_preferred3(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportTypeOnlyPreferred3") {
        return;
    }
    let content = r#"// @module: esnext
// @moduleResolution: bundler
// @Filename: /a.ts
export class A {}
export class B {}
// @Filename: /b.ts
let x: A/*b*/;
// @Filename: /c.ts
import { A } from "./a";
new A();
let x: B/*c*/;
// @Filename: /d.ts
new A();
let x: B;
// @Filename: /ns.ts
export * as default from "./a";
// @Filename: /e.ts
let x: /*e*/ns.A;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "b");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import type { A } from "./a";

let x: A;"#
            .to_string()],
        Some(UserPreferences {
            prefer_type_only_auto_imports: core::TSTrue,
            ..Default::default()
        }),
    );
    f.go_to_marker(t, "c");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { A, type B } from "./a";
new A();
let x: B;"#
            .to_string()],
        Some(UserPreferences {
            prefer_type_only_auto_imports: core::TSTrue,
            ..Default::default()
        }),
    );
    f.go_to_file(t, "/d.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { A, type B } from "./a";

new A();
let x: B;"#
                .to_string(),
        },
    );
    f.go_to_marker(t, "e");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import type ns from "./ns";

let x: ns.A;"#
            .to_string()],
        Some(UserPreferences {
            prefer_type_only_auto_imports: core::TSTrue,
            ..Default::default()
        }),
    );
    done();
}
