#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_services() {
    let mut t = TestingT;
    run_test_js_doc_services(&mut t);
}

fn run_test_js_doc_services(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocServices") {
        return;
    }
    let content = r#"interface /*I*/I {}

/**
 * @param /*use*/[|foo|] I pity the foo
 */
function f([|[|/*def*/{| "contextRangeIndex": 1 |}foo|]: I|]) {
    return /*use2*/[|foo|];
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "use");
    f.verify_quick_info_is(t, "(parameter) foo: I", "I pity the foo");
    f.verify_baseline_find_all_references(
        t,
        &["use".to_string(), "def".to_string(), "use2".to_string()],
    );
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[0].clone().into(),
            f.ranges()[2].clone().into(),
            f.ranges()[3].clone().into(),
        ],
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        vec![
            MarkerOrRangeOrName::Range(f.ranges()[0].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[2].clone()),
            MarkerOrRangeOrName::Range(f.ranges()[3].clone()),
        ],
    );
    f.verify_baseline_go_to_type_definition(t, &["use".to_string()]);
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
