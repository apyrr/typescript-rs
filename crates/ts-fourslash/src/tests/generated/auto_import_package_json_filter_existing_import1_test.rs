#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_json_filter_existing_import1() {
    let mut t = TestingT;
    run_test_auto_import_package_json_filter_existing_import1(&mut t);
}

fn run_test_auto_import_package_json_filter_existing_import1(t: &mut TestingT) {
    if should_skip_if_failing("TestAutoImportPackageJsonFilterExistingImport1") {
        return;
    }
    let content = r#"// @lib: es5
// @module: preserve
// @Filename: /home/src/workspaces/project/node_modules/@types/react/index.d.ts
export declare function useMemo(): void;
export declare function useState(): void;
// @Filename: /home/src/workspaces/project/package.json
{}
// @Filename: /home/src/workspaces/project/index.ts
import { useState } from "react";
useMemo/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"import { useMemo, useState } from "react";
useMemo"#
            .to_string()],
        None,
    );
    f.delete_line(t, 0);
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(t, &[], None);
    done();
}
