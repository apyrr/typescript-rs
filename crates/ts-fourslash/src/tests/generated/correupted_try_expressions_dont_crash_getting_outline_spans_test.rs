#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_correupted_try_expressions_dont_crash_getting_outline_spans() {
    let mut t = TestingT;
    run_test_correupted_try_expressions_dont_crash_getting_outline_spans(&mut t);
}

fn run_test_correupted_try_expressions_dont_crash_getting_outline_spans(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"try[| {
  var x = [
    {% try[||] %}|][|{% except %}|] 
  ]
} catch (e)[| {
  
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
