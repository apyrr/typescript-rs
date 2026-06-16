#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_header_comment1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_header_comment1(&mut t);
}

fn run_test_import_name_code_fix_header_comment1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /a.ts
export const foo = 0;
// @Filename: /b.ts
export const bar = 0;
// @Filename: /c.ts
/*--------------------
 *  Copyright Header
 *--------------------*/

import { bar } from "./b";
foo;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/c.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"/*--------------------
 *  Copyright Header
 *--------------------*/

import { foo } from "./a";
import { bar } from "./b";
foo;"#
                .to_string(),
        ],
        None,
    );
    done();
}
