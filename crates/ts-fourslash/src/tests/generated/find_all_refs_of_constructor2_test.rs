#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_of_constructor2() {
    let mut t = TestingT;
    run_test_find_all_refs_of_constructor2(&mut t);
}

fn run_test_find_all_refs_of_constructor2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class A {
    /*a*/constructor(s: string) {}
}
class B extends A {
    /*b*/constructor() { super(""); }
}
class C extends B {
    /*c*/constructor() {
        super();
    }
}
class D extends B { }
const a = new A("a");
const b = new B();
const c = new C();
const d = new D();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["a".to_string(), "b".to_string(), "c".to_string()]);
    done();
}
