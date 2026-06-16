#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_private_name_accessors() {
    let mut t = TestingT;
    run_test_find_all_refs_private_name_accessors(&mut t);
}

fn run_test_find_all_refs_private_name_accessors(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsPrivateNameAccessors") {
        return;
    }
    let content = r"class C {
    /*1*/get /*2*/#foo(){ return 1; }
    /*3*/set /*4*/#foo(value: number){  }
    constructor() {
        this./*5*/#foo();
    }
}
class D extends C {
    constructor() {
        super()
        this.#foo = 20;
    }
}
class E {
    /*6*/get /*7*/#foo(){ return 1; }
    /*8*/set /*9*/#foo(value: number){  }
    constructor() {
        this./*10*/#foo();
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
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
        ],
    );
    done();
}
