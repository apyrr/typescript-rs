#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_all_promote_type() {
    let mut t = TestingT;
    run_test_import_name_code_fix_all_promote_type(&mut t);
}

fn run_test_import_name_code_fix_all_promote_type(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_all_promoteType") {
        return;
    }
    let content = r"// @Filename: /a.ts
export class A {}
export class B {}
export class C {}
export class D {}
export class E {}
export class F {}
export class G {}
// @Filename: /b.ts
import type { A, C, D, E, G } from './a';
type Z = B | A;
new F;
// @Filename: /c.ts
import type { A, C, D, E, G } from './a';
type Z = B | A;
type Y = F;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r"import { B, F, type A, type C, type D, type E, type G } from './a';
type Z = B | A;
new F;"
                .to_string(),
        },
    );
    f.go_to_file(t, "/c.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r"import type { A, B, C, D, E, F, G } from './a';
type Z = B | A;
type Y = F;"
                .to_string(),
        },
    );
    done();
}
