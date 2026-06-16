#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_property_access_expression_heritage_clause() {
    let mut t = TestingT;
    run_test_rename_property_access_expression_heritage_clause(&mut t);
}

fn run_test_rename_property_access_expression_heritage_clause(t: &mut TestingT) {
    if should_skip_if_failing("TestRenamePropertyAccessExpressionHeritageClause") {
        return;
    }
    let content = r#"class B {}
function foo() {
    return {[|[|{| "contextRangeIndex": 0 |}B|]: B|]};
}
class C extends (foo()).[|B|] {}
class C1 extends foo().[|B|] {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "B");
    done();
}
