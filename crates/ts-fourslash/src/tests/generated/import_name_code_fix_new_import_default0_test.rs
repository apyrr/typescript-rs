#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_default0() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_default0(&mut t);
}

fn run_test_import_name_code_fix_new_import_default0(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"[|f1/*0*/();|]
// @Filename: module.ts
export default function f1() { };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import f1 from "./module";

f1();"#
                .to_string(),
        ],
        None,
    );
    done();
}
