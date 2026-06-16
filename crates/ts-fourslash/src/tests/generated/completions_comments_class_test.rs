#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_comments_class() {
    let mut t = TestingT;
    run_test_completions_comments_class(&mut t);
}

fn run_test_completions_comments_class(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsCommentsClass") {
        return;
    }
    let content = r#"// @lib: es5
/** This is class c2 without constructor*/
class c2 {
}
var i2 = new c2();
var i2_c = c2;
class c3 {
    /** Constructor comment*/
    constructor() {
    }
}
var i3 = new c3();
var i3_c = c3;
/** Class comment*/
class c4 {
    /** Constructor comment*/
    constructor() {
    }
}
var i4 = new c4();
var i4_c = c4;
/** Class with statics*/
class c5 {
    static s1: number;
}
var i5 = new c5();
var i5_c = c5;
/** class with statics and constructor*/
class c6 {
    /** s1 comment*/
    static s1: number;
    /** constructor comment*/
    constructor() {
    }
}
var i6 = new c6();
var i6_c = c6;
/*26*/
class a {
    /**
    constructor for a
    @param a this is my a
    */
    constructor(a: string) {
    }
}
new a("Hello");
namespace m {
    export namespace m2 {
        /** class comment */
        export class c1 {
            /** constructor comment*/
            constructor() {
            }
        }
    }
}
var myVar = new m.m2.c1();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
