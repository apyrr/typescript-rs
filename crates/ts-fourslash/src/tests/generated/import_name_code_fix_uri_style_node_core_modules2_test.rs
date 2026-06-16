#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_uri_style_node_core_modules2() {
    let mut t = TestingT;
    run_test_import_name_code_fix_uri_style_node_core_modules2(&mut t);
}

fn run_test_import_name_code_fix_uri_style_node_core_modules2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @module: commonjs
// @Filename: /node_modules/@types/node/index.d.ts
declare module "fs" { function writeFile(): void }
declare module "fs/promises" { function writeFile(): Promise<void> }
declare module "node:fs" { export * from "fs"; }
declare module "node:fs/promises" { export * from "fs/promises"; }
// @Filename: /other.ts
import "node:fs/promises";
// @Filename: /index.ts
writeFile/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["node:fs".to_string(), "node:fs/promises".to_string()],
        None,
    );
    f.go_to_file(t, "/other.ts");
    f.replace_line(t, 0, "\n");
    f.go_to_file(t, "/index.ts");
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec![
            "fs".to_string(),
            "fs/promises".to_string(),
            "node:fs".to_string(),
            "node:fs/promises".to_string(),
        ],
        None,
    );
    f.go_to_file(t, "/other.ts");
    f.replace_line(t, 0, "import \"node:fs/promises\";\n");
    f.go_to_file(t, "/index.ts");
    f.verify_import_fix_module_specifiers(
        t,
        "",
        &vec!["node:fs".to_string(), "node:fs/promises".to_string()],
        None,
    );
    done();
}
