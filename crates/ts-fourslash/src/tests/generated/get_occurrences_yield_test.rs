#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_yield() {
    let mut t = TestingT;
    run_test_get_occurrences_yield(&mut t);
}

fn run_test_get_occurrences_yield(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesYield") {
        return;
    }
    let content = r"function* f() {
 [|yield|] 100;
 [|y/**/ield|] [|yield|] 200;
  class Foo {
      *memberFunction() {
          return yield 1;
      }
  }
  return function* g() {
    yield 1;
  }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    done();
}
