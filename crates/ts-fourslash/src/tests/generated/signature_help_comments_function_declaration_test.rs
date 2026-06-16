#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_comments_function_declaration() {
    let mut t = TestingT;
    run_test_signature_help_comments_function_declaration(&mut t);
}

fn run_test_signature_help_comments_function_declaration(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpCommentsFunctionDeclaration") {
        return;
    }
    let content = r#"/** This comment should appear for foo*/
function foo() {
}
foo(/*4*/);
/** This is comment for function signature*/
function fooWithParameters(/** this is comment about a*/a: string,
    /** this is comment for b*/
    b: number) {
    var d = a;
}
fooWithParameters(/*10*/"a",/*11*/10);
/**
* Does something
* @param a a string
*/
declare function fn(a: string);
fn(/*12*/"hello");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
