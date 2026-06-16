#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_in_constructors_exact_match() {
    let mut t = TestingT;
    run_test_navigation_items_in_constructors_exact_match(&mut t);
}

fn run_test_navigation_items_in_constructors_exact_match(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noLib: true
class Test {
    [|private search1: number;|]
    constructor([|public search2: boolean|], [|readonly search3: string|], search4: string) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[workspace_symbol_case(
        "search",
        vec![
            symbol_information(
                "search1",
                lsproto::SymbolKindProperty,
                f.ranges()[0].ls_location(),
                Some("Test"),
            ),
            symbol_information(
                "search2",
                lsproto::SymbolKindProperty,
                f.ranges()[1].ls_location(),
                Some("Test"),
            ),
            symbol_information(
                "search3",
                lsproto::SymbolKindProperty,
                f.ranges()[2].ls_location(),
                Some("Test"),
            ),
        ],
    )]);
    done();
}
