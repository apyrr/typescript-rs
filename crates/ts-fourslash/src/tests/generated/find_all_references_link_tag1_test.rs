#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_link_tag1() {
    let mut t = TestingT;
    run_test_find_all_references_link_tag1(&mut t);
}

fn run_test_find_all_references_link_tag1(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesLinkTag1") {
        return;
    }
    let content = r"class C/*7*/ {
    m/*1*/() { }
    n/*2*/ = 1
    static s/*3*/() { }
    /**
     * {@link m}
     * @see {m}
     * {@link C.m}
     * @see {C.m}
     * {@link C#m}
     * @see {C#m}
     * {@link C.prototype.m}
     * @see {C.prototype.m}
     */
    p() { }
    /**
     * {@link n}
     * @see {n}
     * {@link C.n}
     * @see {C.n}
     * {@link C#n}
     * @see {C#n}
     * {@link C.prototype.n}
     * @see {C.prototype.n}
     */
    q() { }
    /**
     * {@link s}
     * @see {s}
     * {@link C.s}
     * @see {C.s}
     */
    r() { }
}

interface I/*8*/ {
    a/*4*/()
    b/*5*/: 1
    /**
     * {@link a}
     * @see {a}
     * {@link I.a}
     * @see {I.a}
     * {@link I#a}
     * @see {I#a}
     */
    c()
    /**
     * {@link b}
     * @see {b}
     * {@link I.b}
     * @see {I.b}
     */
    d()
}

function nestor() {
    /** {@link r2} */
    function ref() { }
    /** @see {r2} */
    function d3() { }
    function r2/*6*/() { }
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
        ],
    );
    done();
}
