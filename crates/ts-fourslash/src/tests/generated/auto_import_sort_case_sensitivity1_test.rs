#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_sort_case_sensitivity1() {
    let mut t = TestingT;
    run_test_auto_import_sort_case_sensitivity1(&mut t);
}

fn run_test_auto_import_sort_case_sensitivity1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /exports1.ts
export const a = 0;
export const A = 1;
export const b = 2;
export const B = 3;
export const c = 4;
export const C = 5;
// @Filename: /exports2.ts
export const d = 0;
export const D = 1;
export const e = 2;
export const E = 3;
// @Filename: /index0.ts
import { A, B, C } from "./exports1";
a/*0*/
// @Filename: /index1.ts
import { A, a, B, b } from "./exports1";
import { E } from "./exports2";
d/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "0");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C } from "./exports1";
a"#
            .to_string(),
        ],
        None,
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { a, A, B, C } from "./exports1";
a"#
            .to_string(),
        ],
        None,
    );
    f.go_to_marker(t, "1");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, a, B, b } from "./exports1";
import { d, E } from "./exports2";
d"#
            .to_string(),
        ],
        None,
    );
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { A, a, B, b } from "./exports1";
import { E, d } from "./exports2";
d"#
            .to_string(),
        ],
        None,
    );
    done();
}
