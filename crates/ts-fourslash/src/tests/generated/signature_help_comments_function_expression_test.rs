#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_comments_function_expression() {
    let mut t = TestingT;
    run_test_signature_help_comments_function_expression(&mut t);
}

fn run_test_signature_help_comments_function_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpCommentsFunctionExpression") {
        return;
    }
    let content = r#"/** lambdaFoo var comment*/
var lambdaFoo = /** this is lambda comment*/ (/**param a*/a: number, /**param b*/b: number) => a + b;
var lambddaNoVarComment = /** this is lambda multiplication*/ (/**param a*/a: number, /**param b*/b: number) => a * b;
lambdaFoo(/*5*/10, /*6*/20);
function anotherFunc(a: number) {
    /** documentation
        @param b {string} inner parameter */
    var lambdaVar = /** inner docs */(b: string) => {
        var localVar = "Hello ";
        return localVar + b;
    }
    return lambdaVar("World") + a;
}
/**
 * On variable
 * @param s the first parameter!
 * @returns the parameter's length
 */
var assigned = /**
                * Summary on expression
                * @param s param on expression
                * @returns return on expression
                */function(/** On parameter */s: string) {
  return s.length;
}
assigned(/*18*/"hey");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
