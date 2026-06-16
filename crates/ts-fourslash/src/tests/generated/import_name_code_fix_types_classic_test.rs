#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_types_classic() {
    let mut t = TestingT;
    run_test_import_name_code_fix_types_classic(&mut t);
}

fn run_test_import_name_code_fix_types_classic(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @moduleResolution: classic
// @Filename: /node_modules/@types/foo/index.d.ts
export const xyz: number;
// @Filename: /node_modules/bar/index.d.ts
export const qrs: number;
// @Filename: /a.ts
xyz;
qrs;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.ts");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { xyz } from "foo";
import { qrs } from "./node_modules/bar/index";

xyz;
qrs;"#
                .to_string(),
        },
    );
    done();
}
