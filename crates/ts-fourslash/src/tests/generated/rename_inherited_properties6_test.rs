#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_inherited_properties6() {
    let mut t = TestingT;
    run_test_rename_inherited_properties6(&mut t);
}

fn run_test_rename_inherited_properties6(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameInheritedProperties6") {
        return;
    }
    let content = r#"interface C extends D {
    propD: number;
}
interface D extends C {
    [|[|{| "contextRangeIndex": 0 |}propC|]: number;|]
}
var d: D;
d.[|propC|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "propC");
    done();
}
