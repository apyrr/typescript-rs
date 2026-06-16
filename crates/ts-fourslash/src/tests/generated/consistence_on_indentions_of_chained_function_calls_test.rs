#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_consistence_on_indentions_of_chained_function_calls() {
    let mut t = TestingT;
    run_test_consistence_on_indentions_of_chained_function_calls(&mut t);
}

fn run_test_consistence_on_indentions_of_chained_function_calls(t: &mut TestingT) {
    if should_skip_if_failing("TestConsistenceOnIndentionsOfChainedFunctionCalls") {
        return;
    }
    let content = r"interface ig {
  module(data): ig;
   requires(data): ig;
   defines(data): ig;
}

var ig: ig;
ig.module(
   'mything'
).requires(
   'otherstuff'
).defines(/*0*//*1*/
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.go_to_marker(t, "0");
    f.verify_indentation(t, 4);
    done();
}
