#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_for_regions() {
    let mut t = TestingT;
    run_test_get_outlining_spans_for_regions(&mut t);
}

fn run_test_get_outlining_spans_for_regions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// region without label
[|// #region

// #endregion|]

// region without label with trailing spaces
[|// #region  

// #endregion|]

// region with label
[|// #region label1

// #endregion|]

// region with extra whitespace in all valid locations
             [|//              #region          label2    label3

        //        #endregion|]

// No space before directive
[|//#region label4

//#endregion|]

// Nested regions
[|// #region outer

[|// #region inner

// #endregion inner|]

// #endregion outer|]

// region delimiters not valid when there is preceding text on line
 test // #region invalid1

test // #endregion

// region delimiters not valid when in multiline comment
/*
// #region invalid2
*/

/*
// #endregion
*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
