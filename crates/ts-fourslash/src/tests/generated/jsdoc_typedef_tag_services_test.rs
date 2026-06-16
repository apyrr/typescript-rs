#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_services() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_services(&mut t);
}

fn run_test_jsdoc_typedef_tag_services(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagServices") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
/**
 * Doc comment
 * [|@typedef /*def*/[|{| "contextRangeIndex": 0 |}Product|]
 * @property {string} title
 |]*/
/**
 * @type {[|/*use*/Product|]}
 */
const product = null;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "use",
        "type Product = {\n    title: string;\n}",
        "Doc comment",
    );
    f.verify_baseline_find_all_references(t, &["use".to_string(), "def".to_string()]);
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.ranges()[1..].iter().cloned().map(Into::into).collect(),
    );
    f.verify_baseline_document_highlights(
        t,
        None,
        f.ranges()
            .into_iter()
            .map(MarkerOrRangeOrName::Range)
            .collect(),
    );
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
