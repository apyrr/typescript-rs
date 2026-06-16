#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_for_regions_no_single_line_folds() {
    let mut t = TestingT;
    run_test_get_outlining_spans_for_regions_no_single_line_folds(&mut t);
}

fn run_test_get_outlining_spans_for_regions_no_single_line_folds(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
[|//#region
function foo()[| {

}|]
[|//these
//should|]
//#endregion not you|]
[|// be
// together|]

[|//#region bla bla bla

function bar()[| { }|]

//#endregion|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_outlining_spans_from_ranges(t);
    done();
}
