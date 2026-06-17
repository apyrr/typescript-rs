#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_js_cj_svs_esm3() {
    let mut t = TestingT;
    run_test_import_name_code_fix_js_cj_svs_esm3(&mut t);
}

fn run_test_import_name_code_fix_js_cj_svs_esm3(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_jsCJSvsESM3") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: types/dep.d.ts
export declare class Dep {}
// @Filename: index.js
import fs from 'fs';
const path = require('path');

Dep/**/
// @Filename: util2.js
export {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![r"import fs from 'fs';
import { Dep } from './types/dep';
const path = require('path');

Dep"
        .to_string()],
        None,
    );
    done();
}
