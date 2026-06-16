#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_deduplicate_duplicate_merged_bind_check_errors() {
    let mut t = TestingT;
    run_test_deduplicate_duplicate_merged_bind_check_errors(&mut t);
}

fn run_test_deduplicate_duplicate_merged_bind_check_errors(t: &mut TestingT) {
    if should_skip_if_failing("TestDeduplicateDuplicateMergedBindCheckErrors") {
        return;
    }
    let content = r"class X {
  foo() {
      return 1;
  }
  get foo() {
      return 1;
  }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(2);
    done();
}
