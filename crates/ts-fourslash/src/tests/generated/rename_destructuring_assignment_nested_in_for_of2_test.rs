#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_destructuring_assignment_nested_in_for_of2() {
    let mut t = TestingT;
    run_test_rename_destructuring_assignment_nested_in_for_of2(&mut t);
}

fn run_test_rename_destructuring_assignment_nested_in_for_of2(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameDestructuringAssignmentNestedInForOf2") {
        return;
    }
    let content = r#"interface MultiRobot {
    name: string;
    skills: {
        [|[|{| "contextRangeIndex": 0 |}primary|]: string;|]
        secondary: string;
    };
}
let multiRobots: MultiRobot[], [|[|{| "contextRangeIndex": 2 |}primary|]: string|];
for ([|{ skills: { [|{| "contextRangeIndex": 4 |}primary|]: primaryA, secondary: secondaryA } } of multiRobots|]) {
    console.log(primaryA);
}
for ([|{ skills: { [|{| "contextRangeIndex": 6 |}primary|], secondary } } of multiRobots|]) {
    console.log([|primary|]);
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[7].clone().into(),
            f.ranges()[8].clone().into(),
        ],
    );
    done();
}
