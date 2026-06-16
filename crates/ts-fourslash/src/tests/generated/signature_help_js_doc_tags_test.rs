#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_js_doc_tags() {
    let mut t = TestingT;
    run_test_signature_help_js_doc_tags(&mut t);
}

fn run_test_signature_help_js_doc_tags(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpJSDocTags") {
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
var foo = new Foo(/*10*/4);
Foo.method1(/*11*/);
foo.method2(/*12*/);
foo.method3(/*13*/);
foo.method4();
foo.property1;
foo.property2;
foo.method5();
foo.newMet";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
