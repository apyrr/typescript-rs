#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_export_specifier2() {
    let mut t = TestingT;
    run_test_rename_export_specifier2(&mut t);
}

fn run_test_rename_export_specifier2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: a.ts
const name = {};
export { name/**/ };
// @Filename: b.ts
import { name } from './a';
const x = name.toString();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
