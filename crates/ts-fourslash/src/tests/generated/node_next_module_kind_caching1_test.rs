#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_node_next_module_kind_caching1() {
    let mut t = TestingT;
    run_test_node_next_module_kind_caching1(&mut t);
}

fn run_test_node_next_module_kind_caching1(t: &mut TestingT) {
    if should_skip_if_failing("TestNodeNextModuleKindCaching1") {
        return;
    }
    let content = r#"// @Filename: tsconfig.json
{
    "compilerOptions": {
      "lib": ["es5"],
      "rootDir": "src",
      "outDir": "dist",
      "target": "ES2020",
      "module": "NodeNext",
      "strict": true
    },
    "include": ["src\\**\\*.ts"]
}
// @Filename: package.json
{
    "type": "module",
    "private": true
}
// @Filename: src/index.ts
// The line below should show a "Relative import paths need explicit file
// extensions..." error in VS Code, but it doesn't. The error is only picked up
// by `tsc` which seems to properly infer the module type.
import { helloWorld } from './example'
/**/
helloWorld()
// @Filename: src/example.ts
export function helloWorld() {
    console.log('Hello, world!')
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
