#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_destructuring_declaration_in_for() {
    let mut t = TestingT;
    run_test_rename_destructuring_declaration_in_for(&mut t);
}

fn run_test_rename_destructuring_declaration_in_for(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameDestructuringDeclarationInFor") {
        return;
    }
    let content = r#"interface I {
    [|[|{| "contextRangeIndex": 0 |}property1|]: number;|]
    property2: string;
}
var elems: I[];

var p2: number, property1: number;
for ([|let { [|{| "contextRangeIndex": 2 |}property1|]: p2 } = elems[0]|]; p2 < 100; p2++) {
}
for ([|let { [|{| "contextRangeIndex": 4 |}property1|] } = elems[0]|]; p2 < 100; p2++) {
    [|property1|] = p2;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[6].clone().into(),
        ],
    );
    done();
}
