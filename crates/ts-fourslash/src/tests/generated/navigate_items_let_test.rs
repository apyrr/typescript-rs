#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigate_items_let() {
    let mut t = TestingT;
    run_test_navigate_items_let(&mut t);
}

fn run_test_navigate_items_let(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noLib: true
let [|c = 10|];
function foo() {
    let [|d = 10|];
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[
        workspace_symbol_case(
            "c",
            vec![symbol_information(
                "c",
                lsproto::SymbolKindVariable,
                f.ranges()[0].ls_location(),
                None,
            )],
        ),
        workspace_symbol_case(
            "d",
            vec![symbol_information(
                "d",
                lsproto::SymbolKindVariable,
                f.ranges()[1].ls_location(),
                Some("foo"),
            )],
        ),
    ]);
    done();
}
