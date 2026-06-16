#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navto_exclude_lib3() {
    let mut t = TestingT;
    run_test_navto_exclude_lib3(&mut t);
}

fn run_test_navto_exclude_lib3(t: &mut TestingT) {
    if should_skip_if_failing("TestNavto_excludeLib3") {
        return;
    }
    let content = r"// @filename: /index.ts
[|function parseInt(s: string): number {}|]";
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[workspace_symbol_case_with_preferences(
        "parseInt",
        vec![symbol_information(
            "parseInt",
            lsproto::SymbolKindFunction,
            f.ranges()[0].ls_location(),
            None,
        )],
        Some(UserPreferences {
            exclude_library_symbols_in_nav_to: Some(true),
            ..Default::default()
        }),
    )]);
    done();
}
