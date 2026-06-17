#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type7() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type7(&mut t);
}

fn run_test_import_name_code_fix_import_type7(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_importType7") {
        return;
    }
    let content = r#"// @module: es2015
// @Filename: /exports.ts
export interface SomeInterface {}
export class SomePig {}
// @Filename: /a.ts
import {
    type SomeInterface,
    type SomePig,
} from "./exports.js";
new SomePig/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import {
    SomePig,
    type SomeInterface,
} from "./exports.js";
new SomePig"#
            .to_string()],
        None,
    );
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import {
    SomePig,
    type SomeInterface,
} from "./exports.js";
new SomePig"#
            .to_string()],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Last,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import {
    type SomeInterface,
    SomePig,
} from "./exports.js";
new SomePig"#
            .to_string()],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::Inline,
            ..Default::default()
        }),
    );
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import {
    type SomeInterface,
    SomePig,
} from "./exports.js";
new SomePig"#
            .to_string()],
        Some(UserPreferences {
            organize_imports_type_order: lsutil::OrganizeImportsTypeOrder::First,
            ..Default::default()
        }),
    );
    done();
}
