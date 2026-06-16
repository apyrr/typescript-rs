#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigate_to_symbol_iterator() {
    let mut t = TestingT;
    run_test_navigate_to_symbol_iterator(&mut t);
}

fn run_test_navigate_to_symbol_iterator(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
class C {
    [|[Symbol.iterator]() {}|]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[workspace_symbol_case(
        "iterator",
        vec![symbol_information(
            "iterator",
            lsproto::SymbolKindMethod,
            f.ranges()[0].ls_location(),
            Some("C"),
        )],
    )]);
    done();
}
