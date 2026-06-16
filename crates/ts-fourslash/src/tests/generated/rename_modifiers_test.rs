#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_modifiers() {
    let mut t = TestingT;
    run_test_rename_modifiers(&mut t);
}

fn run_test_rename_modifiers(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameModifiers") {
        return;
    }
    let content = r#"[|[|declare|] [|abstract|] class [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -3 |}C1|] {
    [|[|static|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}a|];|]
    [|[|readonly|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}b|];|]
    [|[|public|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}c|];|]
    [|[|protected|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}d|];|]
    [|[|private|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}e|];|]
}|]
[|[|const|] enum [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}E|] {
}|]
[|[|async|] function [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -2 |}fn|]() {}|]
[|[|export|] [|default|] class [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeDelta": -3 |}C2|] {}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[2].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[8].clone().into(),
            f.ranges()[11].clone().into(),
            f.ranges()[14].clone().into(),
            f.ranges()[17].clone().into(),
            f.ranges()[20].clone().into(),
            f.ranges()[23].clone().into(),
            f.ranges()[26].clone().into(),
            f.ranges()[27].clone().into(),
        ],
    );
    done();
}
