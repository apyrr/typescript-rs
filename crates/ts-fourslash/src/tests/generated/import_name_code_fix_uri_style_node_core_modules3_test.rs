#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_uri_style_node_core_modules3() {
    let mut t = TestingT;
    run_test_import_name_code_fix_uri_style_node_core_modules3(&mut t);
}

fn run_test_import_name_code_fix_uri_style_node_core_modules3(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_uriStyleNodeCoreModules3") {
        return;
    }
    let content = r#"// @module: commonjs
// @Filename: /node_modules/@types/node/index.d.ts
declare module "path" { function join(...segments: readonly string[]): string; }
declare module "node:path" { export * from "path"; }
declare module "fs" { function writeFile(): void }
declare module "fs/promises" { function writeFile(): Promise<void> }
declare module "node:fs" { export * from "fs"; }
declare module "node:fs/promises" { export * from "fs/promises"; }
// @Filename: /other.ts
import "node:fs/promises";
// @Filename: /noPrefix.ts
import "path";
writeFile/*noPrefix*/
// @Filename: /prefix.ts
import "node:path";
writeFile/*prefix*/
// @Filename: /mixed1.ts
import "path";
import "node:path";
writeFile/*mixed1*/
// @Filename: /mixed2.ts
import "node:path";
import "path";
writeFile/*mixed2*/
// @Filename: /test1.ts
import "node:test";
import "path";
writeFile/*test1*/
// @Filename: /test2.ts
import "node:test";
writeFile/*test2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_module_specifiers(
        t,
        "noPrefix",
        &vec!["fs".to_string(), "fs/promises".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "prefix",
        &vec!["node:fs".to_string(), "node:fs/promises".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "mixed1",
        &vec!["node:fs".to_string(), "node:fs/promises".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "mixed2",
        &vec!["node:fs".to_string(), "node:fs/promises".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "test1",
        &vec!["fs".to_string(), "fs/promises".to_string()],
        None,
    );
    f.verify_import_fix_module_specifiers(
        t,
        "test2",
        &vec!["node:fs".to_string(), "node:fs/promises".to_string()],
        None,
    );
    done();
}
