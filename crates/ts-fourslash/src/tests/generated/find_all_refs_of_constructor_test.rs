#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_of_constructor() {
    let mut t = TestingT;
    run_test_find_all_refs_of_constructor(&mut t);
}

fn run_test_find_all_refs_of_constructor(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsOfConstructor") {
        return;
    }
    let content = r#"class A {
    /*aCtr*/constructor(s: string) {}
}
class B extends A { }
class C extends B {
    /*cCtr*/constructor() {
        super("");
    }
}
class D extends B { }
class E implements A { }
const a = new A("a");
const b = new B("b");
const c = new C();
const d = new D("d");
const e = new E();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["aCtr".to_string(), "cCtr".to_string()]);
    done();
}
