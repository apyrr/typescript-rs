#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigate_to_import() {
    let mut t = TestingT;
    run_test_navigate_to_import(&mut t);
}

fn run_test_navigate_to_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
// @Filename: library.ts
[|export function foo() {}|]
[|export function bar() {}|]
// @Filename: user.ts
import {foo, [|bar as baz|]} from './library';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[
        workspace_symbol_case(
            "foo",
            vec![symbol_information(
                "foo",
                lsproto::SymbolKindFunction,
                f.ranges()[0].ls_location(),
                None,
            )],
        ),
        workspace_symbol_case(
            "bar",
            vec![symbol_information(
                "bar",
                lsproto::SymbolKindFunction,
                f.ranges()[1].ls_location(),
                None,
            )],
        ),
        workspace_symbol_case(
            "baz",
            vec![symbol_information(
                "baz",
                lsproto::SymbolKindVariable,
                f.ranges()[2].ls_location(),
                None,
            )],
        ),
    ]);
    done();
}
