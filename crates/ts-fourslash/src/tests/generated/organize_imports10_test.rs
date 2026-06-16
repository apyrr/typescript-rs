#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports10() {
    let mut t = TestingT;
    run_test_organize_imports10(&mut t);
}

fn run_test_organize_imports10(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports10") {
        return;
    }
    let content = r"// @Filename: /module.ts
import type { ZodType } from './declaration';

/** Intended to be used in combination with {@link ZodType} */
export function fun() { /* ... */ }
// @Filename: /declaration.ts
 type ZodType = {};
 export type { ZodType }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r"import type { ZodType } from './declaration';

/** Intended to be used in combination with {@link ZodType} */
export function fun() { /* ... */ }",
        "source.organizeImports",
        None,
    );
    done();
}
