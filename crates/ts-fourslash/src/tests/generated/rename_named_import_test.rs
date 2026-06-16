#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_named_import() {
    let mut t = TestingT;
    run_test_rename_named_import(&mut t);
}

fn run_test_rename_named_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/lib/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/lib/index.ts
const unrelatedLocalVariable = 123;
export const someExportedVariable = unrelatedLocalVariable;
// @Filename: /home/src/workspaces/project/src/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/src/index.ts
import { /*i*/someExportedVariable } from '../lib/index';
someExportedVariable;
// @Filename: /home/src/workspaces/project/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] } }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/home/src/workspaces/project/lib/index.ts");
    f.go_to_file(t, "/home/src/workspaces/project/src/index.ts");
    f.verify_baseline_rename(t, &["i".to_string()]);
    done();
}
