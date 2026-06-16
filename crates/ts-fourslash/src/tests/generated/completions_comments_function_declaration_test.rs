#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_comments_function_declaration() {
    let mut t = TestingT;
    run_test_completions_comments_function_declaration(&mut t);
}

fn run_test_completions_comments_function_declaration(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsCommentsFunctionDeclaration") {
        return;
    }
    let content = r#"// @lib: es5
/** This comment should appear for foo*/
function foo() {
}
foo/*3*/();
/** This is comment for function signature*/
function fooWithParameters(/** this is comment about a*/a: string,
    /** this is comment for b*/
    b: number) {
    var d = /*7*/a;
}
fooWithParameters/*9*/("a",10);
/**
* Does something
* @param a a string
*/
declare function fn(a: string);
fn("hello");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
