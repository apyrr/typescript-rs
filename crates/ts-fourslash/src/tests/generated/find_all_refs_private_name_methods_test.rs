#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_private_name_methods() {
    let mut t = TestingT;
    run_test_find_all_refs_private_name_methods(&mut t);
}

fn run_test_find_all_refs_private_name_methods(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    /*1*/#foo(){ }
    constructor() {
        this./*2*/#foo();
    }
}
class D extends C {
    constructor() {
        super()
        this.#foo = 20;
    }
}
class E {
    /*3*/#foo(){ }
    constructor() {
        this./*4*/#foo();
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
