#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_package_json_filter_existing_import3() {
    let mut t = TestingT;
    run_test_auto_import_package_json_filter_existing_import3(&mut t);
}

fn run_test_auto_import_package_json_filter_existing_import3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/tsconfig.json
{ "compilerOptions": { "lib": ["es5"], "module": "preserve", "types": ["*"] } }
// @Filename: /home/src/workspaces/project/node_modules/@types/node/index.d.ts
declare module "node:fs" {
    export function readFile(): void;
    export function writeFile(): void;
}
// @Filename: /home/src/workspaces/project/package.json
{}
// @Filename: /home/src/workspaces/project/index.ts
readFile/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(t, &[], None);
    f.go_to_bof(t);
    f.insert_line(t, "import { writeFile } from \"node:fs\";");
    f.go_to_marker(t, "");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { readFile, writeFile } from "node:fs";
readFile"#
                .to_string(),
        ],
        None,
    );
    done();
}
