#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_prefix_match() {
    let mut t = TestingT;
    run_test_navigation_items_prefix_match(&mut t);
}

fn run_test_navigation_items_prefix_match(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationItemsPrefixMatch") {
        return;
    }
    let content = r#"// @noLib: true
[|{| "name": "Shapes", "kind": "module" |}namespace Shapes {
    [|{| "name": "Point", "kind": "class", "kindModifiers": "export", "containerName": "Shapes", "containerKind": "module" |}export class Point {
        [|{| "name": "originality", "kind": "property", "kindModifiers": "private", "containerName": "Point", "containerKind": "class" |}private originality = 0.0;|]

        [|{| "name": "distanceFromOrig", "kind": "property", "kindModifiers": "private", "containerName": "Point", "containerKind": "class" |}private distanceFromOrig = 0.0;|]

        [|{| "name": "distanceFarFarAway", "kind": "getter", "containerName": "Point", "containerKind": "class" |}get distanceFarFarAway(): number { return 0; }|]
    }|]
}|]

var [|{| "name": "xyz", "kind": "var" |}xyz = new Shapes.Point()|];"#;
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for range in f.ranges() {
        f.verify_workspace_symbol(&[workspace_symbol_case_from_range_with_pattern(&range, {
            let name = range_marker_data(&range).data.get("name").unwrap();
            name[..name.len() - 1].to_string()
        })]);
    }
    done();
}
