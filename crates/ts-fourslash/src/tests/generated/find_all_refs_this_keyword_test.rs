#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_this_keyword() {
    let mut t = TestingT;
    run_test_find_all_refs_this_keyword(&mut t);
}

fn run_test_find_all_refs_this_keyword(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsThisKeyword") {
        return;
    }
    let content = r"// @noLib: true
/*1*/this;
function f(/*2*/this) {
    return /*3*/this;
    function g(/*4*/this) { return /*5*/this; }
}
class C {
    static x() {
        /*6*/this;
    }
    static y() {
        () => /*7*/this;
    }
    constructor() {
        /*8*/this;
    }
    method() {
        () => /*9*/this;
    }
}
// These are *not* real uses of the 'this' keyword, they are identifiers.
const x = { /*10*/this: 0 }
x./*11*/this;";
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
            "11".to_string(),
        ],
    );
    done();
}
