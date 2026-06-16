#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_require_import_vs_require_add_to_existing_wins() {
    let mut t = TestingT;
    run_test_import_name_code_fix_require_import_vs_require_add_to_existing_wins(&mut t);
}

fn run_test_import_name_code_fix_require_import_vs_require_add_to_existing_wins(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_require_importVsRequire_addToExistingWins") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: blah.js
export default class Blah {}
export const Named1 = 0;
export const Named2 = 1;
// @Filename: index.js
var path = require('path')
  , { promisify } = require('util')
  , { Named1 } = require('./blah')

import fs from 'fs'

new Blah";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.js");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Update import from \"./blah\"".to_string(),
            new_file_content: r"var path = require('path')
  , { promisify } = require('util')
  , { Named1, default: Blah } = require('./blah')

import fs from 'fs'

new Blah"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
