#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_function_in_namespace_with_trivia() {
    let mut t = TestingT;
    run_test_unused_function_in_namespace_with_trivia(&mut t);
}

fn run_test_unused_function_in_namespace_with_trivia(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedFunctionInNamespaceWithTrivia") {
        return;
    }
    let content = r"// @noUnusedLocals: true
[| namespace greeter {
  // Do not remove
  /**
   * JSDoc Comment
   */
  function function1() {
  }/*1*/
} |]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "namespace greeter {\n    // Do not remove\n }",
        false,
        0,
        0,
    );
    done();
}
