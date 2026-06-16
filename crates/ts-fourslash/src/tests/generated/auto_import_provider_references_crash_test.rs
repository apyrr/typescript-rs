#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_auto_import_provider_references_crash() {
    let mut t = TestingT;
    run_test_auto_import_provider_references_crash(&mut t);
}

fn run_test_auto_import_provider_references_crash(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /home/src/workspaces/project/a/package.json
{}
// @Filename: /home/src/workspaces/project/a/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/a/index.ts
class A {}
// @Filename: /home/src/workspaces/project/a/index.d.ts
declare class A {
}
//# sourceMappingURL=index.d.ts.map
// @Filename: /home/src/workspaces/project/a/index.d.ts.map
{"version":3,"file":"index.d.ts","sourceRoot":"","sources":["index.ts"],"names":[],"mappings":"AAAA,OAAO,OAAO,CAAC;CAAG"}
// @Filename: /home/src/workspaces/project/b/tsconfig.json
{
  "compilerOptions": { "disableSourceOfProjectReferenceRedirect": true, "lib": ["es5"] },
  "references": [{ "path": "../a" }]
}
// @Filename: /home/src/workspaces/project/b/b.ts
/// <reference path="../a/index.d.ts" />
new A/**/();
// @Filename: /home/src/workspaces/project/c/package.json
{ "dependencies": { "a": "*" } }
// @Filename: /home/src/workspaces/project/c/tsconfig.json
{ "compilerOptions": { "lib": ["es5"] }, "references" [{ "path": "../a" }] }
// @Filename: /home/src/workspaces/project/c/index.ts
export {};
// @link: /home/src/workspaces/project/a -> /home/src/workspaces/project/c/node_modules/a"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_file(t, "/home/src/workspaces/project/c/index.ts");
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
