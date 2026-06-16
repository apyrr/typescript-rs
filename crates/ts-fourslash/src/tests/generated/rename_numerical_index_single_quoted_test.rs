#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_numerical_index_single_quoted() {
    let mut t = TestingT;
    run_test_rename_numerical_index_single_quoted(&mut t);
}

fn run_test_rename_numerical_index_single_quoted(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const foo = { [|0|]: true };
foo[[|0|]];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "0");
    done();
}
