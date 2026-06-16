#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_alias_external_module() {
    let mut t = TestingT;
    run_test_rename_alias_external_module(&mut t);
}

fn run_test_rename_alias_external_module(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameAliasExternalModule") {
        return;
    }
    let content = r#"// @Filename: a.ts
namespace SomeModule { export class SomeClass { } }
export = SomeModule;
// @Filename: b.ts
[|import [|{| "contextRangeIndex": 0 |}M|] = require("./a");|]
import C = [|M|].SomeClass;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "M");
    done();
}
