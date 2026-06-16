#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_js_cj_svs_esm2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_js_cj_svs_esm2(&mut t);
}

fn run_test_import_name_code_fix_js_cj_svs_esm2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: types/dep.d.ts
export declare class Dep {}
// @Filename: index.js
Dep/**/
// @Filename: util1.ts
import fs from 'fs';
// @Filename: util2.js
const fs = require('fs');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"const { Dep } = require("./types/dep");

Dep"#
                .to_string(),
        ],
        None,
    );
    done();
}
