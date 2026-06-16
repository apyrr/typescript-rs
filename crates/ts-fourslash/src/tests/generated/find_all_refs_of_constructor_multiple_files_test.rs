#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_of_constructor_multiple_files() {
    let mut t = TestingT;
    run_test_find_all_refs_of_constructor_multiple_files(&mut t);
}

fn run_test_find_all_refs_of_constructor_multiple_files(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsOfConstructor_multipleFiles") {
        return;
    }
    let content = r#"// @Filename: f.ts
class A {
    /*aCtr*/constructor(s: string) {}
}
class B extends A { }
export { A, B };
// @Filename: a.ts
import { A as A1 } from "./f";
const a1 = new A1("a1");
export default class extends A1 { }
export { B as B1 } from "./f";
// @Filename: b.ts
import B, { B1 } from "./a";
const d = new B("b");
const d1 = new B1("b1");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["aCtr".to_string()]);
    done();
}
