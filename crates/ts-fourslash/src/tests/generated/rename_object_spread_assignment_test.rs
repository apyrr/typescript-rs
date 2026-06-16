#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_object_spread_assignment() {
    let mut t = TestingT;
    run_test_rename_object_spread_assignment(&mut t);
}

fn run_test_rename_object_spread_assignment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface A1 { a: number };
interface A2 { a?: number };
[|let [|{| "contextRangeIndex": 0 |}a1|]: A1;|]
[|let [|{| "contextRangeIndex": 2 |}a2|]: A2;|]
let a12 = { ...[|a1|], ...[|a2|] };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[4].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
        ],
    );
    done();
}
