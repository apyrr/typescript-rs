#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_file_quote_style1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_file_quote_style1(&mut t);
}

fn run_test_import_name_code_fix_new_import_file_quote_style1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|import { v2 } from "./module2";

f1/*0*/();|]
// @Filename: module1.ts
export function f1() {}
// @Filename: module2.ts
export var v2 = 6;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { f1 } from "./module1";
import { v2 } from "./module2";

f1();"#
                .to_string(),
        ],
        None,
    );
    done();
}
