#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_trailing_comma() {
    let mut t = TestingT;
    run_test_import_name_code_fix_trailing_comma(&mut t);
}

fn run_test_import_name_code_fix_trailing_comma(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: index.ts
import {
  T2,
  T1,
} from "./types";

const x: T3/**/
// @Filename: types.ts
export type T1 = 0;
export type T2 = 0;
export type T3 = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import {
  T2,
  T1,
  T3,
} from "./types";

const x: T3"#
                .to_string(),
        ],
        None,
    );
    done();
}
