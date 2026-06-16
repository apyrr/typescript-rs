#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_js_cj_svs_esm1() {
    let mut t = TestingT;
    run_test_import_name_code_fix_js_cj_svs_esm1(&mut t);
}

fn run_test_import_name_code_fix_js_cj_svs_esm1(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_jsCJSvsESM1") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: types/dep.d.ts
export declare class Dep {}
// @Filename: index.js
Dep/**/
// @Filename: util.js
import fs from 'fs';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { Dep } from "./types/dep";

Dep"#
                .to_string(),
        ],
        None,
    );
    done();
}
