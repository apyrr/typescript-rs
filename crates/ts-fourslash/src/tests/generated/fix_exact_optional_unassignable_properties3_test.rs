#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_fix_exact_optional_unassignable_properties3() {
    let mut t = TestingT;
    run_test_fix_exact_optional_unassignable_properties3(&mut t);
}

fn run_test_fix_exact_optional_unassignable_properties3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strictNullChecks: true
// @exactOptionalPropertyTypes: true
// @Filename: fixExactOptionalUnassignableProperties2.ts
import { INodeModules } from 'foo'
interface J {
    a?: number | undefined
}
declare var inm: INodeModules
declare var j: J
inm/**/ = j
console.log(inm)
// @Filename: node_modules/@types/foo/index.d.ts
export interface INodeModules {
    a?: number
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
