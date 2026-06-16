#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_generate_definitions() {
    let mut t = TestingT;
    run_test_code_fix_generate_definitions(&mut t);
}

fn run_test_code_fix_generate_definitions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/foo/index.d.ts
module.exports = 0;
// @Filename: /a.ts
import * as foo from "foo";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
