#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_namespace_and_shorthand() {
    let mut t = TestingT;
    run_test_rename_import_namespace_and_shorthand(&mut t);
}

fn run_test_rename_import_namespace_and_shorthand(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|import * as [|{| "contextRangeIndex": 0 |}foo|] from 'bar';|]
const bar = { [|foo|] };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[2].clone().into()],
    );
    done();
}
