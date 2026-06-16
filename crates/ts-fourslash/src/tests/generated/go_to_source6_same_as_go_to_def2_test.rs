#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_source6_same_as_go_to_def2() {
    let mut t = TestingT;
    run_test_go_to_source6_same_as_go_to_def2(&mut t);
}

fn run_test_go_to_source6_same_as_go_to_def2(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToSource6_sameAsGoToDef2") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: /home/src/workspaces/project/node_modules/foo/package.json
{ "name": "foo", "version": "1.2.3", "typesVersions": { "*": { "*": ["./types/*"] } } }
// @Filename: /home/src/workspaces/project/node_modules/foo/src/a.ts
export const /*end*/a = 'a';
// @Filename: /home/src/workspaces/project/node_modules/foo/types/a.d.ts
export declare const a: string;
//# sourceMappingURL=a.d.ts.map
// @Filename: /home/src/workspaces/project/node_modules/foo/types/a.d.ts.map
{"version":3,"file":"a.d.ts","sourceRoot":"","sources":["../src/a.ts"],"names":[],"mappings":"AAAA,eAAO,MAAM,EAAE,OAAO,CAAC;;AACvB,wBAAsB"}
// @Filename: /home/src/workspaces/project/node_modules/foo/dist/a.js
export const a = 'a';
// @Filename: /home/src/workspaces/project/b.ts
import { a } from 'foo/a';
[|a/*start*/|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_source_definition(t, &["start".to_string()]);
    f.verify_baseline_go_to_definition(t, &["start".to_string()]);
    done();
}
