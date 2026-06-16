#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_convert_to_type_only_import1() {
    let mut t = TestingT;
    run_test_code_fix_convert_to_type_only_import1(&mut t);
}

fn run_test_code_fix_convert_to_type_only_import1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: esnext
// @verbatimModuleSyntax: true
// @Filename: exports.ts
export default class A {}
export class B {}
export class C {}
// @Filename: imports.ts
import {
    B,
    C,
} from './exports';

declare const b: B;
declare const c: C;
console.log(b, c);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "imports.ts");
    f.verify_code_fix_not_available(t, &[]);
    done();
}
