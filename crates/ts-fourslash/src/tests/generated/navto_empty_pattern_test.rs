#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navto_empty_pattern() {
    let mut t = TestingT;
    run_test_navto_empty_pattern(&mut t);
}

fn run_test_navto_empty_pattern(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @filename: foo.ts
const [|x: number = 1|];
[|function y(x: string): string { return x; }|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[workspace_symbol_case(
        "",
        vec![
            symbol_information(
                "x",
                lsproto::SymbolKindVariable,
                f.ranges()[0].ls_location(),
                None,
            ),
            symbol_information(
                "y",
                lsproto::SymbolKindFunction,
                f.ranges()[1].ls_location(),
                None,
            ),
        ],
    )]);
    done();
}
