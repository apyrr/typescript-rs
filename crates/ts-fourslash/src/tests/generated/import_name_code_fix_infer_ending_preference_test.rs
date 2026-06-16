#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_infer_ending_preference() {
    let mut t = TestingT;
    run_test_import_name_code_fix_infer_ending_preference(&mut t);
}

fn run_test_import_name_code_fix_infer_ending_preference(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: esnext
// @moduleResolution: bundler
// @Filename: /a.mts
export {};
// @Filename: /b.ts
export {};
// @Filename: /c.ts
export const c = 0;
// @Filename: /main.ts
import {} from "./a.mjs";
import {} from "./b";

c/**/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(t, "", &vec!["./c".to_string()], None);
    done();
}
