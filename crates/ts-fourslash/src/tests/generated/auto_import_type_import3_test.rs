#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_type_import3() {
    let mut t = TestingT;
    run_test_auto_import_type_import3(&mut t);
}

fn run_test_auto_import_type_import3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @verbatimModuleSyntax: true
// @target: esnext
// @Filename: /foo.ts
export const A = 1;
export type B = { x: number };
export type C = 1;
export class D = { y: string };
// @Filename: /test.ts
import { A, type B, type C } from './foo';
const b: B | C;
console.log(A, D/**/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r"import { A, D, type B, type C } from './foo';
const b: B | C;
console.log(A, D);"
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r"import { A, type B, type C, D } from './foo';
const b: B | C;
console.log(A, D);"
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r"import { A, type B, type C, D } from './foo';
const b: B | C;
console.log(A, D);"
                .to_string(),
        ],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    done();
}
