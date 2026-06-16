#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_outlining_spans_for_arguments() {
    let mut t = TestingT;
    run_test_outlining_spans_for_arguments(&mut t);
}

fn run_test_outlining_spans_for_arguments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"console.log(123, 456)l;
console.log(
);
console.log[|(
    123, 456
)|];
console.log[|(
    123,
    456
)|];
() =>[| console.log[|(
    123,
    456
)|]|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
