#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_class_in_namespace_with_trivia1() {
    let mut t = TestingT;
    run_test_unused_class_in_namespace_with_trivia1(&mut t);
}

fn run_test_unused_class_in_namespace_with_trivia1(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedClassInNamespaceWithTrivia1") {
        return;
    }
    let content = r"// @noUnusedLocals: true
[| namespace greeter {
  /* comment1 */
  class /* comment2 */ class1 {
  }
} |]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "namespace greeter {\n   /* comment1 */\n}", false, 0, 0);
    done();
}
