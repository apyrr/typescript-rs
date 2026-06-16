#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_destructuring_declaration_in_for_of() {
    let mut t = TestingT;
    run_test_rename_destructuring_declaration_in_for_of(&mut t);
}

fn run_test_rename_destructuring_declaration_in_for_of(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface I {
    [|[|{| "contextRangeIndex": 0 |}property1|]: number;|]
    property2: string;
}
var elems: I[];

for ([|let { [|{| "contextRangeIndex": 2 |}property1|] } of elems|]) {
    [|property1|]++;
}
for ([|let { [|{| "contextRangeIndex": 5 |}property1|]: p2 } of elems|]) {
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[6].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[4].clone().into(),
        ],
    );
    done();
}
