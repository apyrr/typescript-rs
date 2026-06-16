#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_import_specifier_property_name() {
    let mut t = TestingT;
    run_test_rename_import_specifier_property_name(&mut t);
}

fn run_test_rename_import_specifier_property_name(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameImportSpecifierPropertyName") {
        return;
    }
    let content = r"// @Filename: canada.ts
export interface /**/Ginger {}
// @Filename: dry.ts
import { Ginger as Ale } from './canada';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
