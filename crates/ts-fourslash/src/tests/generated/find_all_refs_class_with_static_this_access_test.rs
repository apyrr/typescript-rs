#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_class_with_static_this_access() {
    let mut t = TestingT;
    run_test_find_all_refs_class_with_static_this_access(&mut t);
}

fn run_test_find_all_refs_class_with_static_this_access(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|class /*0*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}C|] {
    static s() {
        /*1*/[|this|];
    }
    static get f() {
        return /*2*/[|this|];

        function inner() { this; }
        class Inner { x = this; }
    }
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[1].clone().into()]);
    done();
}
