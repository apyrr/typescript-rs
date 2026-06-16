#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navto01() {
    let mut t = TestingT;
    run_test_navto01(&mut t);
}

fn run_test_navto01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
/// Module
[|{| "name": "MyShapes", "kind": "module" |}namespace MyShapes {

    // Class
    [|{| "name": "MyPoint", "kind": "class", "kindModifiers": "export", "containerName": "MyShapes", "containerKind": "module" |}export class MyPoint {
        // Instance member
        [|{| "name": "MyoriginAttheHorizon", "kind": "property", "kindModifiers": "private", "containerName": "MyPoint", "containerKind": "class" |}private MyoriginAttheHorizon = 0.0;|]

        // Getter
        [|{| "name": "MydistanceFromOrigin", "kind": "getter", "containerName": "MyPoint", "containerKind": "class" |}get MydistanceFromOrigin(): number { return 0; }|]
    }|]
}|]

// Local variables
var [|{| "name": "myXyz", "kind": "var" |}myXyz = new Shapes.Point()|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    for range in f.ranges() {
        f.verify_workspace_symbol(&[workspace_symbol_case_from_range_with_pattern(&range, {
            let name = range_marker_data(&range).data.get("name").unwrap();
            name[2..].to_string()
        })]);
    }
    done();
}
