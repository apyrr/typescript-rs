#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_interactive_parameter_names_with_comments() {
    let mut t = TestingT;
    run_test_inlay_hints_interactive_parameter_names_with_comments(&mut t);
}

fn run_test_inlay_hints_interactive_parameter_names_with_comments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const fn = (x: any) => { }
fn(/* nobody knows exactly what this param is */ 42);
function foo (aParameter: number, bParameter: number, cParameter: number) { }
foo(
    /** aParameter */
    1,
    // bParameter
    2,
    /* cParameter */
    3
)
foo(
    /** multiple comments */
    /** aParameter */
    1,
    /** bParameter */
    /** multiple comments */
    2,
    // cParameter
    /** multiple comments */
    3
)
foo(
    /** wrong name */
    1,
    2,
    /** multiple */
    /** wrong */
    /** name */
    3
)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
