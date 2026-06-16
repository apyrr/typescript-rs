#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_on_import_aliases() {
    let mut t = TestingT;
    run_test_find_all_refs_on_import_aliases(&mut t);
}

fn run_test_find_all_refs_on_import_aliases(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsOnImportAliases") {
        return;
    }
    let content = r#"//@Filename: a.ts
export class /*0*/Class {
}
//@Filename: b.ts
import { /*1*/Class } from "./a";

var c = new /*2*/Class();
//@Filename: c.ts
export { /*3*/Class } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
