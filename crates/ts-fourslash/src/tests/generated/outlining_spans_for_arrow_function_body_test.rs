#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_outlining_spans_for_arrow_function_body() {
    let mut t = TestingT;
    run_test_outlining_spans_for_arrow_function_body(&mut t);
}

fn run_test_outlining_spans_for_arrow_function_body(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"() => 42;
() => ( 42 );
() =>[| {
    42
}|];
() => [|(
    42
)|];
() =>[| "foo" +
    "bar" +
    "baz"|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
