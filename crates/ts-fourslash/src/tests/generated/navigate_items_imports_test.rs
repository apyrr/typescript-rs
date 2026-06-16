#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigate_items_imports() {
    let mut t = TestingT;
    run_test_navigate_items_imports(&mut t);
}

fn run_test_navigate_items_imports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noLib: true
import [|{| "name": "ns", "kind": "alias" |}* as ns|] from "a";

import { [|{| "name": "a", "kind": "alias" |}a|] }  from "a";

import { [|{| "name": "B", "kind": "alias" |}b as B|] }  from "a";

import { [|{| "name": "c", "kind": "alias" |}c|],
            [|{| "name": "dee", "kind": "alias" |}d as dee|] }  from "a";

import [|{| "name": "d1", "kind": "alias" |}d1|], {
            [|{| "name": "e", "kind": "alias" |}e|] }  from "a";

[|{| "name": "f", "kind": "alias" |}import f = require("a");|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for range in f.ranges() {
        f.verify_workspace_symbol(&[workspace_symbol_case_from_range_with_pattern(
            &range,
            range_marker_data(&range)
                .data
                .get("name")
                .unwrap()
                .to_string(),
        )]);
    }
    done();
}
