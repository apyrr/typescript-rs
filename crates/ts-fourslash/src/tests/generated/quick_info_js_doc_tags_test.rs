#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags(&mut t);
}

fn run_test_quick_info_js_doc_tags(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJSDocTags") {
        return;
    }
    let content = r"/**
 * This is class Foo.
 * @mytag comment1 comment2
 */
class Foo {
    /**
     * This is the constructor.
     * @myjsdoctag this is a comment
     */
    constructor(value: number) {}
    /**
     * method1 documentation
     * @mytag comment1 comment2
     */
    static method1() {}
    /**
     * @mytag
     */
    method2() {}
    /**
     * @mytag comment1 comment2
     */
    property1: string;
    /**
     * @mytag1 some comments
     * some more comments about mytag1
     * @mytag2
     * here all the comments are on a new line
     * @mytag3
     * @mytag
     */
    property2: number;
    /**
     * @returns {number} a value
     */
    method3(): number { return 3; }
    /**
     * @param {string} foo A value.
     * @returns {number} Another value
     * @mytag
     */
    method4(foo: string): number { return 3; }
    /** @mytag */
    method5() {}
    /** method documentation
     *  @mytag a JSDoc tag
     */
    newMethod() {}
}
var foo = new /*1*/Foo(/*10*/4);
/*2*/Foo./*3*/method1(/*11*/);
foo./*4*/method2(/*12*/);
foo./*5*/method3(/*13*/);
foo./*6*/method4();
foo./*7*/property1;
foo./*8*/property2;
foo./*9*/method5();
foo.newMet/*14*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
