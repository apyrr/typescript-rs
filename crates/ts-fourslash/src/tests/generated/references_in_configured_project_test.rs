#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_in_configured_project() {
    let mut t = TestingT;
    run_test_references_in_configured_project(&mut t);
}

fn run_test_references_in_configured_project(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesInConfiguredProject") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/referencesForGlobals_1.ts
class /*0*/globalClass {
    public f() { }
}
// @Filename: /home/src/workspaces/project/referencesForGlobals_2.ts
var c = /*1*/globalClass();
// @Filename: /home/src/workspaces/project/tsconfig.json
{ "files": ["referencesForGlobals_1.ts", "referencesForGlobals_2.ts"], "compilerOptions": { "lib": ["es5"] } }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
