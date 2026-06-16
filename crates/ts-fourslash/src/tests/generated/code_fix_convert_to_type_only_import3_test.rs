#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_convert_to_type_only_import3() {
    let mut t = TestingT;
    run_test_code_fix_convert_to_type_only_import3(&mut t);
}

fn run_test_code_fix_convert_to_type_only_import3(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixConvertToTypeOnlyImport3") {
        return;
    }
    let content = r#"// @module: esnext
// @verbatimModuleSyntax: true
// @Filename: exports1.ts
export default class A {}
export class B {}
export class C {}
// @Filename: exports2.ts
export default class D {}
export class E {}
export class F {}
// @Filename: imports.ts
import A, { B, C } from './exports1';
import D, * as others from "./exports2";

declare const a: A;
declare const b: B;
declare const c: C;
declare const d: D;
declare const o: typeof others;
console.log(a, b, c, d, o);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "imports.ts");
    f.verify_code_fix_not_available(t, &[]);
    done();
}
