#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_destructuring_nested_binding_element() {
    let mut t = TestingT;
    run_test_rename_destructuring_nested_binding_element(&mut t);
}

fn run_test_rename_destructuring_nested_binding_element(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameDestructuringNestedBindingElement") {
        return;
    }
    let content = r#"interface MultiRobot {
    name: string;
    skills: {
        [|[|{| "contextRangeIndex": 0|}primary|]: string;|]
        secondary: string;
    };
}
let multiRobots: MultiRobot[];
for ([|let { skills: {[|{| "contextRangeIndex": 2|}primary|]: primaryA, secondary: secondaryA } } of multiRobots|]) {
    console.log(primaryA);
}
for ([|let { skills: {[|{| "contextRangeIndex": 4|}primary|], secondary } } of multiRobots|]) {
    console.log([|primary|]);
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
