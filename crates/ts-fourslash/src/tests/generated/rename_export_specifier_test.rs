#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_export_specifier() {
    let mut t = TestingT;
    run_test_rename_export_specifier(&mut t);
}

fn run_test_rename_export_specifier(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameExportSpecifier") {
        return;
    }
    let content = r"// @Filename: a.ts
const name = {};
export { name as name/**/ };
// @Filename: b.ts
import { name } from './a';
const x = name.toString();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
