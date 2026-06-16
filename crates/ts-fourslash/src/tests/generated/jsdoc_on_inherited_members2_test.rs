#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_on_inherited_members2() {
    let mut t = TestingT;
    run_test_jsdoc_on_inherited_members2(&mut t);
}

fn run_test_jsdoc_on_inherited_members2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocOnInheritedMembers2") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @filename: /a.js
/** @template T */
class A {
    /** Method documentation. */
    method() {}
}

/** @extends {A<number>} */
const B = class extends A {
    method() {}
}

const b = new B();
b.method/**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
